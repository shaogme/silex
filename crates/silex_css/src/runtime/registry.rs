use crate::layers;
use crate::runtime::backend::{self, ActiveSheet, SheetBackend, SheetHandle};
use crate::runtime::platform::{report, schedule_microtask};
use std::{cell::RefCell, collections::HashSet};

/// 一次文档增删。
#[derive(Debug)]
pub(crate) enum DocOp {
    /// 静态表只有一张，且必须排在最前面（层序声明在它里面）
    SetStatic(SheetHandle),
    Add(SheetHandle),
    Remove(SheetHandle),
}

thread_local! {
    pub(crate) static DOCUMENT_REGISTRY: RefCell<DocumentStyleRegistry> = RefCell::new(DocumentStyleRegistry::new());
    /// `DOCUMENT_REGISTRY` 正被借用时来不及做的增删。
    ///
    /// `DynamicStyleState::drop` 此前是 `if let Ok(mut dr) = try_borrow_mut()`，
    /// 借不到就直接跳过 `remove_sheet`——那张样式表就**永久**留在
    /// `document.adoptedStyleSheets` 上了，且没有任何提示。
    ///
    /// 增与删共用一个队列而不是各排各的：同一张表可能先被摘、后被挂回（退休后
    /// 复用），两个队列各自 drain 就丢了它们之间的先后。
    static PENDING_OPS: RefCell<Vec<DocOp>> = const { RefCell::new(Vec::new()) };
}

/// 拿到文档级注册表，顺带把欠下的增删补上。
pub(crate) fn with_document_registry<R>(
    f: impl FnOnce(&mut DocumentStyleRegistry) -> R,
) -> Option<R> {
    // `try_with` 而不是 `with`：这条路会被 `Drop` 走到，而 `Drop` 可能发生在
    // 线程退出时的 TLS 析构里——那时注册表本身可能已经没了。做不了就当作
    // 「借不到」，由调用方排队；排不上也无所谓，进程都要结束了。
    DOCUMENT_REGISTRY
        .try_with(|dr| {
            let Ok(mut dr) = dr.try_borrow_mut() else {
                return None;
            };
            let owed = PENDING_OPS
                .try_with(|p| p.borrow_mut().drain(..).collect::<Vec<_>>())
                .unwrap_or_default();
            for op in owed {
                dr.apply(op);
            }
            Some(f(&mut dr))
        })
        .ok()
        .flatten()
}

/// 做一笔文档增删：拿得到注册表就地做，借不到就排进队列、约一个微任务回来补做。
///
/// 走同一个入口是为了不再有「借不到就算了」的分支——此前 `attach` 是直接
/// `return`，那张表要等到退休之后又被复用才会重试，在那之前它的样式一直不生效。
pub(crate) fn apply_doc_op(op: DocOp) {
    let mut pending = Some(op);
    // 借不到时闭包根本没被调用，`pending` 里的那笔操作还在
    let applied = with_document_registry(|dr| {
        if let Some(op) = pending.take() {
            dr.apply(op);
        }
    })
    .is_some();

    if applied {
        return;
    }
    let Some(op) = pending else { return };
    report(match &op {
        DocOp::SetStatic(_) => "注册静态样式表时借用冲突，已排入下一个微任务",
        DocOp::Add(_) => "挂载动态样式表时借用冲突，已排入下一个微任务",
        DocOp::Remove(_) => "摘除动态样式表时借用冲突，已排入下一个微任务",
    });
    if PENDING_OPS.try_with(|p| p.borrow_mut().push(op)).is_err() {
        return;
    }
    schedule_microtask(|| {
        with_document_registry(|dr| dr.sync());
    });
}

/// A global registry for static styles to avoid duplicated styles.
/// It merges all static styles into a single shared Constructable StyleSheet.
pub struct StaticStyleRegistry {
    /// Set of already injected style IDs.
    injected_ids: HashSet<String>,
    /// IDs queued for the next flush but not accepted by the backend yet.
    pending_ids: HashSet<String>,
    /// The shared stylesheet for all static styles.
    shared_sheet: Option<ActiveSheet>,
    /// 已经进表的 chunk。只在需要整表重建（`<style>` 兜底 / `insertRule` 失败）
    /// 时才会被读到。
    all_chunks: Vec<String>,
    /// 还没进表的 chunk。
    pending_chunks: Vec<(String, String)>,
    /// A failed full replacement forces the next flush to retry the complete table.
    needs_full_rebuild: bool,
    /// Whether a microtask flush has already been scheduled.
    is_flush_pending: bool,
}

thread_local! {
    static STATIC_REGISTRY: RefCell<StaticStyleRegistry> = RefCell::new(StaticStyleRegistry {
        injected_ids: HashSet::new(),
        pending_ids: HashSet::new(),
        shared_sheet: None,
        all_chunks: Vec::new(),
        pending_chunks: Vec::new(),
        needs_full_rebuild: false,
        is_flush_pending: false,
    });
    /// `STATIC_REGISTRY` 被重入借用时暂存的注入请求。
    ///
    /// 此前 `StaticStyleRegistry::with` 借不到就返回 `None`，而 `inject_style`
    /// 根本不看返回值——那段 CSS 就**静默消失**了。
    static DEFERRED_INJECTIONS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// 把注册表恢复成刚启动的样子。
///
/// 测试用。`--test-threads=1` 时 libtest 会在同一个线程上跑多个测试，thread_local
/// 里的状态会串场，所以每个状态机测试都要先从这里开一张白纸。
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn reset_for_test() {
    DEFERRED_INJECTIONS.with(|d| d.borrow_mut().clear());
    PENDING_OPS.with(|p| p.borrow_mut().clear());
    STATIC_REGISTRY.with(|r| {
        *r.borrow_mut() = StaticStyleRegistry {
            injected_ids: HashSet::new(),
            pending_ids: HashSet::new(),
            shared_sheet: None,
            all_chunks: Vec::new(),
            pending_chunks: Vec::new(),
            needs_full_rebuild: false,
            is_flush_pending: false,
        }
    });
    DOCUMENT_REGISTRY.with(|dr| *dr.borrow_mut() = DocumentStyleRegistry::new());
}

impl StaticStyleRegistry {
    /// 拿到静态注册表，顺带把欠下的注入补上。借不到时返回 `None`。
    pub(crate) fn with<R>(f: impl FnOnce(&mut Self) -> R) -> Option<R> {
        STATIC_REGISTRY.with(|i| {
            let Ok(mut reg) = i.try_borrow_mut() else {
                return None;
            };
            let owed = DEFERRED_INJECTIONS.with(|d| d.borrow_mut().drain(..).collect::<Vec<_>>());
            for (id, content) in &owed {
                reg.inject(id, content);
            }
            Some(f(&mut reg))
        })
    }

    pub fn inject(&mut self, id: &str, content: &str) {
        if self.injected_ids.contains(id) {
            return;
        }
        if self.pending_ids.contains(id) {
            // 上一轮后端拒绝了完整刷新；同一个 id 的再次注入是一个明确的
            // 重试信号，但不重复追加同一 chunk。
            self.schedule_flush();
            return;
        }

        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        if self.shared_sheet.is_none() {
            let Some(sheet) = ActiveSheet::create() else {
                report("无法创建静态样式表，静态样式将不会生效");
                return;
            };
            // 层序声明必须是表里的第一条规则，后续 chunk 一律追加在它后面
            if !sheet.replace(layers::ORDER_STATEMENT) {
                report("无法初始化静态样式表，静态样式将不会生效");
                return;
            }
            if let Some(adopted) = sheet.adopted() {
                apply_doc_op(DocOp::SetStatic(adopted));
            }
            self.shared_sheet = Some(sheet);
        }

        self.pending_ids.insert(id.to_string());
        self.pending_chunks
            .push((id.to_string(), trimmed.to_string()));
        self.schedule_flush();
    }

    fn schedule_flush(&mut self) {
        if self.is_flush_pending || self.pending_chunks.is_empty() {
            return;
        }
        self.is_flush_pending = true;

        schedule_microtask(|| {
            if StaticStyleRegistry::with(|r| r.flush()).is_none() {
                report("刷新静态样式表时借用冲突");
            }
        });
    }

    /// 把待处理的 chunk 追加进样式表。
    ///
    /// 此前每次 flush 都把**所有** chunk 重新拼成一个大字符串再 `replaceSync`，
    /// 浏览器整表重新解析一遍；组件在不同 tick 陆续挂载时总成本是 O(n²)。
    /// 现在只把新增的那几条规则 `insertRule` 到表尾。
    pub fn flush(&mut self) {
        self.is_flush_pending = false;
        if self.pending_chunks.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.pending_chunks);

        let Some(sheet) = &self.shared_sheet else {
            // 正常路径不会在有 pending chunk 时缺少 shared sheet；保留请求以便
            // 后续重试，而不是把尚未接受的内容误标成已注入。
            self.pending_chunks = pending;
            return;
        };

        let appended = if self.needs_full_rebuild {
            false
        } else {
            let rules: Vec<&str> = pending
                .iter()
                .flat_map(|(_, chunk)| split_rules(chunk))
                .collect();
            sheet.append_rules(&rules)
        };

        if !appended {
            // `<style>` 兜底或某条规则 insertRule 失败：退回整表重建。
            // 兜底路径本来就没有增量接口，O(n²) 在这里是可接受的代价。
            let mut chunks = self.all_chunks.clone();
            chunks.extend(pending.iter().map(|(_, chunk)| chunk.clone()));
            let full = self.build_full_content(&chunks);
            if !sheet.replace(&full) {
                report("重建静态样式表失败，等待下一次注入重试");
                self.pending_chunks = pending;
                self.needs_full_rebuild = true;
                return;
            }
            self.needs_full_rebuild = false;
        }

        self.all_chunks
            .extend(pending.iter().map(|(_, chunk)| chunk.clone()));
        for (id, _) in pending {
            self.pending_ids.remove(&id);
            self.injected_ids.insert(id);
        }
    }

    /// Pre-allocates string capacity and builds the full CSS stylesheet content.
    fn build_full_content(&self, chunks: &[String]) -> String {
        let capacity =
            layers::ORDER_STATEMENT.len() + 1 + chunks.iter().map(|c| c.len() + 1).sum::<usize>();
        let mut full_content = String::with_capacity(capacity);
        full_content.push_str(layers::ORDER_STATEMENT);
        full_content.push('\n');
        for chunk in chunks {
            full_content.push_str(chunk);
            full_content.push('\n');
        }
        full_content
    }
}

/// Helper to split a CSS string into top-level rules.
/// Handles nested blocks (`{}`), strings, escapes, and CSS comments (`/* ... */`).
///
/// 增量注入的前提：`insertRule` 一次只吃一条规则，而一个 chunk（一次 `css!` /
/// `styled!` 的产物）里可能有若干条。
pub fn split_rules(css: &str) -> Vec<&str> {
    let mut rules = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut in_quote = None;
    let mut in_comment = false;
    let bytes = css.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if in_comment {
            if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        match bytes[i] {
            b'/' if in_quote.is_none() && i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                in_comment = true;
                i += 2;
                continue;
            }
            b'\\' => {
                i += 2;
                continue;
            }
            b'"' | b'\'' => {
                let q = bytes[i];
                if in_quote == Some(q) {
                    in_quote = None;
                } else if in_quote.is_none() {
                    in_quote = Some(q);
                }
            }
            b'{' if in_quote.is_none() => depth += 1,
            b'}' if in_quote.is_none() && depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let rule = css[start..i + 1].trim();
                    if !rule.is_empty() {
                        rules.push(rule);
                    }
                    start = i + 1;
                }
            }
            b';' if depth == 0 && in_quote.is_none() => {
                let rule = css[start..i + 1].trim();
                if !rule.is_empty() {
                    rules.push(rule);
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    let tail = css[start..].trim();
    if !tail.is_empty() {
        rules.push(tail);
    }
    rules
}

/// Injects a CSS string into the document.
/// This function uses a shared registry to merge static styles.
pub fn inject_style(id: &str, content: &str) {
    if StaticStyleRegistry::with(|r| r.inject(id, content)).is_none() {
        // 借用冲突（注入过程中又触发了注入）：排队，下一个微任务补做。
        // 此前这里是直接丢弃。
        DEFERRED_INJECTIONS.with(|d| d.borrow_mut().push((id.to_string(), content.to_string())));
        schedule_microtask(|| {
            StaticStyleRegistry::with(|r| r.flush());
        });
    }
}

/// Registry to manage the list of adopted stylesheets in the document.
/// This is the single source of truth for document.adoptedStyleSheets.
pub(crate) struct DocumentStyleRegistry {
    static_sheet: Option<SheetHandle>,
    dynamic_sheets: Vec<SheetHandle>,
    /// 上次真正写进 `document.adoptedStyleSheets` 的那一批表。
    ///
    /// 此前这里存的是 `sheet.unchecked_ref::<JsValue>() as *const _ as usize`
    /// ——**Rust 侧那个 `CssStyleSheet` 值的内存地址**，不是 JS 对象标识。
    /// `dynamic_sheets` 是 `Vec`，元素地址随扩容而变；反过来，同一微任务内
    /// 增删数量相等时新元素可能正好落在同一批槽位上，于是得到完全相同的地址
    /// 集合 → 判定「没变化」→ 跳过同步 → 新样式表永不生效、被移除的永不摘除。
    ///
    /// 现在比的是 `SheetHandle` 的 `PartialEq`，也就是后端定义的对象标识。
    last_synced: Vec<SheetHandle>,
    is_pending: bool,
}

impl DocumentStyleRegistry {
    fn new() -> Self {
        Self {
            static_sheet: None,
            dynamic_sheets: Vec::new(),
            last_synced: Vec::new(),
            is_pending: false,
        }
    }

    fn apply(&mut self, op: DocOp) {
        match op {
            DocOp::SetStatic(sheet) => self.set_static_sheet(sheet),
            DocOp::Add(sheet) => self.add_sheet(sheet),
            DocOp::Remove(sheet) => self.remove_sheet(&sheet),
        }
    }

    pub fn set_static_sheet(&mut self, sheet: SheetHandle) {
        self.static_sheet = Some(sheet);
        self.sync();
    }

    pub fn add_sheet(&mut self, sheet: SheetHandle) {
        self.dynamic_sheets.push(sheet);
        self.sync();
    }

    pub fn remove_sheet(&mut self, sheet: &SheetHandle) {
        self.dynamic_sheets.retain(|s| s != sheet);
        self.sync();
    }

    pub fn sync(&mut self) {
        if self.is_pending {
            return;
        }

        self.is_pending = true;

        schedule_microtask(|| {
            if with_document_registry(|dr| dr.perform_sync()).is_none() {
                report("同步 adoptedStyleSheets 时借用冲突");
            }
        });
    }

    fn perform_sync(&mut self) {
        self.is_pending = false;

        // 静态表永远排在最前：层序声明在它里面，后面的表都靠它定优先级
        let current: Vec<SheetHandle> = self
            .static_sheet
            .iter()
            .chain(self.dynamic_sheets.iter())
            .cloned()
            .collect();

        // 按后端定义的对象标识逐个比对；一致就跳过宿主侧的整表替换。
        if current == self.last_synced {
            return;
        }

        backend::set_adopted(&current);
        self.last_synced = current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 增量注入的前提：一个 chunk 能被切成浏览器 `insertRule` 吃得下的
    /// 单条规则。切错了不是少几条样式，而是整表退回全量重建。
    #[test]
    fn split_rules_separates_top_level_rules() {
        assert_eq!(
            split_rules(".a{color:red}.b{color:blue}"),
            vec![".a{color:red}", ".b{color:blue}"]
        );
    }

    #[test]
    fn split_rules_keeps_nested_blocks_together() {
        assert_eq!(
            split_rules("@media (min-width:1px){.a{color:red}.b{color:blue}}.c{}"),
            vec![
                "@media (min-width:1px){.a{color:red}.b{color:blue}}",
                ".c{}"
            ]
        );
    }

    /// 层序声明是语句式 at-rule，以 `;` 收尾而不是块
    #[test]
    fn split_rules_treats_statements_as_their_own_rule() {
        assert_eq!(
            split_rules("@layer base, components;\n.a{color:red}"),
            vec!["@layer base, components;", ".a{color:red}"]
        );
    }

    /// 字符串里的 `{` `}` `;` 不能被当成结构
    #[test]
    fn split_rules_ignores_braces_inside_strings() {
        assert_eq!(
            split_rules(r#".a{content:"};"}.b{}"#),
            vec![r#".a{content:"};"}"#, ".b{}"]
        );
    }

    #[test]
    fn split_rules_skips_comments() {
        assert_eq!(
            split_rules("/* }{; */.a{color:red}"),
            vec!["/* }{; */.a{color:red}"]
        );
    }
}
