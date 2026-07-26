use crate::layers;
use crate::runtime::sheet::{Sheet, report};
use js_sys::Array;
use silex_dom::prelude::*;
use std::{cell::RefCell, collections::HashSet};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::spawn_local;
use web_sys::CssStyleSheet;

thread_local! {
    pub(crate) static DOCUMENT_REGISTRY: RefCell<DocumentStyleRegistry> = RefCell::new(DocumentStyleRegistry::new());
    /// `DOCUMENT_REGISTRY` 正被借用时来不及做的摘除。
    ///
    /// `DynamicStyleState::drop` 此前是 `if let Ok(mut dr) = try_borrow_mut()`，
    /// 借不到就直接跳过 `remove_sheet`——那张样式表就**永久**留在
    /// `document.adoptedStyleSheets` 上了，且没有任何提示。
    static PENDING_REMOVALS: RefCell<Vec<CssStyleSheet>> = const { RefCell::new(Vec::new()) };
}

/// 拿到文档级注册表，顺带把欠下的摘除补上。
pub(crate) fn with_document_registry<R>(
    f: impl FnOnce(&mut DocumentStyleRegistry) -> R,
) -> Option<R> {
    DOCUMENT_REGISTRY.with(|dr| {
        let Ok(mut dr) = dr.try_borrow_mut() else {
            return None;
        };
        let owed = PENDING_REMOVALS.with(|p| p.borrow_mut().drain(..).collect::<Vec<_>>());
        for sheet in &owed {
            dr.remove_sheet(sheet);
        }
        Some(f(&mut dr))
    })
}

/// 借不到注册表时把摘除排进队列，并约一个微任务回来补做。
pub(crate) fn queue_removal(sheet: CssStyleSheet) {
    PENDING_REMOVALS.with(|p| p.borrow_mut().push(sheet));
    spawn_local(async {
        with_document_registry(|dr| dr.sync());
    });
}

/// A global registry for static styles to avoid duplicated styles.
/// It merges all static styles into a single shared Constructable StyleSheet.
pub struct StaticStyleRegistry {
    /// Set of already injected style IDs.
    injected_ids: HashSet<String>,
    /// The shared stylesheet for all static styles.
    shared_sheet: Option<Sheet>,
    /// 已经进表的 chunk。只在需要整表重建（`<style>` 兜底 / `insertRule` 失败）
    /// 时才会被读到。
    all_chunks: Vec<String>,
    /// 还没进表的 chunk。
    pending_chunks: Vec<String>,
    /// Whether a microtask flush has already been scheduled.
    is_flush_pending: bool,
}

thread_local! {
    static STATIC_REGISTRY: RefCell<StaticStyleRegistry> = RefCell::new(StaticStyleRegistry {
        injected_ids: HashSet::new(),
        shared_sheet: None,
        all_chunks: Vec::new(),
        pending_chunks: Vec::new(),
        is_flush_pending: false,
    });
    /// `STATIC_REGISTRY` 被重入借用时暂存的注入请求。
    ///
    /// 此前 `StaticStyleRegistry::with` 借不到就返回 `None`，而 `inject_style`
    /// 根本不看返回值——那段 CSS 就**静默消失**了。
    static DEFERRED_INJECTIONS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
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

        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        self.injected_ids.insert(id.to_string());
        self.pending_chunks.push(trimmed.to_string());

        if self.shared_sheet.is_none() {
            let Some(sheet) = Sheet::new() else {
                report("无法创建静态样式表，静态样式将不会生效");
                return;
            };
            // 层序声明必须是表里的第一条规则，后续 chunk 一律追加在它后面
            sheet.replace(layers::ORDER_STATEMENT);
            if let Some(adopted) = sheet.adopted() {
                let adopted = adopted.clone();
                if with_document_registry(|dr| dr.set_static_sheet(adopted)).is_none() {
                    report("注册静态样式表时借用冲突，已排入下一个微任务");
                }
            }
            self.shared_sheet = Some(sheet);
        }

        self.schedule_flush();
    }

    fn schedule_flush(&mut self) {
        if self.is_flush_pending || self.pending_chunks.is_empty() {
            return;
        }
        self.is_flush_pending = true;

        spawn_local(async {
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
            // 表都没建起来，内容留在 all_chunks 里等下一次重建
            self.all_chunks.extend(pending);
            return;
        };

        let rules: Vec<&str> = pending.iter().flat_map(|c| split_rules(c)).collect();
        let appended = sheet.append_rules(&rules);

        self.all_chunks.extend(pending);
        if !appended {
            // `<style>` 兜底或某条规则 insertRule 失败：退回整表重建。
            // 兜底路径本来就没有增量接口，O(n²) 在这里是可接受的代价。
            let full = self.build_full_content();
            sheet.replace(&full);
        }
    }

    /// Pre-allocates string capacity and builds the full CSS stylesheet content.
    fn build_full_content(&self) -> String {
        let capacity = layers::ORDER_STATEMENT.len()
            + 1
            + self.all_chunks.iter().map(|c| c.len() + 1).sum::<usize>();
        let mut full_content = String::with_capacity(capacity);
        full_content.push_str(layers::ORDER_STATEMENT);
        full_content.push('\n');
        for chunk in &self.all_chunks {
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
        spawn_local(async {
            StaticStyleRegistry::with(|r| r.flush());
        });
    }
}

/// Registry to manage the list of adopted stylesheets in the document.
/// This is the single source of truth for document.adoptedStyleSheets.
pub(crate) struct DocumentStyleRegistry {
    static_sheet: Option<CssStyleSheet>,
    dynamic_sheets: Vec<CssStyleSheet>,
    /// 上次真正写进 `document.adoptedStyleSheets` 的那一批表。
    ///
    /// 此前这里存的是 `sheet.unchecked_ref::<JsValue>() as *const _ as usize`
    /// ——**Rust 侧那个 `CssStyleSheet` 值的内存地址**，不是 JS 对象标识。
    /// `dynamic_sheets` 是 `Vec`，元素地址随扩容而变；反过来，同一微任务内
    /// 增删数量相等时新元素可能正好落在同一批槽位上，于是得到完全相同的地址
    /// 集合 → 判定「没变化」→ 跳过同步 → 新样式表永不生效、被移除的永不摘除。
    last_synced: Vec<CssStyleSheet>,
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

    pub fn set_static_sheet(&mut self, sheet: CssStyleSheet) {
        self.static_sheet = Some(sheet);
        self.sync();
    }

    pub fn add_sheet(&mut self, sheet: CssStyleSheet) {
        self.dynamic_sheets.push(sheet);
        self.sync();
    }

    pub fn remove_sheet(&mut self, sheet: &CssStyleSheet) {
        let sheet_val: &JsValue = sheet.unchecked_ref();
        self.dynamic_sheets.retain(|s| {
            let s_val: &JsValue = s.unchecked_ref();
            s_val != sheet_val
        });
        self.sync();
    }

    pub fn sync(&mut self) {
        if self.is_pending {
            return;
        }

        self.is_pending = true;

        spawn_local(async {
            if with_document_registry(|dr| dr.perform_sync()).is_none() {
                report("同步 adoptedStyleSheets 时借用冲突");
            }
        });
    }

    fn perform_sync(&mut self) {
        self.is_pending = false;

        let current: Vec<&CssStyleSheet> = self
            .static_sheet
            .iter()
            .chain(self.dynamic_sheets.iter())
            .collect();

        // 按 JS 对象标识逐个比对；一致就跳过浏览器侧的整表替换。
        if current.len() == self.last_synced.len()
            && current.iter().zip(self.last_synced.iter()).all(|(a, b)| {
                let a: &JsValue = a.unchecked_ref();
                let b: &JsValue = b.unchecked_ref();
                a == b
            })
        {
            return;
        }

        let arr: Array = current
            .iter()
            .map(|s| JsValue::from((*s).clone()))
            .collect();
        document().set_adopted_style_sheets(&arr);

        self.last_synced = current.into_iter().cloned().collect();
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
