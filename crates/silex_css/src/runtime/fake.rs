//! 非 wasm 目标下的样式表后端：把每一次调用记进日志，供状态机测试断言。
//!
//! 它只模拟**我们依赖的那部分契约**：表能整体替换、能追加顶层规则、有没有句柄
//! 参与 `adoptedStyleSheets`、以及什么时候真的被 `Drop`。「浏览器对 CSSOM 的
//! 实现是不是这样」不由它保证——那是 wasm 冒烟测试的事。

// 非测试构建里没人读这些记录，但记录本身要一直在：测试与非测试若走两条不同的
// 代码路径，测出来的东西就不作数了。
#![cfg_attr(not(test), allow(dead_code))]

use crate::runtime::backend::{DocumentBackend, SheetBackend};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

/// 后端上发生过的事，按时间顺序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SheetEvent {
    Created(usize),
    Replaced(usize, String),
    Appended(usize, Vec<String>),
    /// 后端拒绝增量追加，调用方应当退回整表替换
    AppendRefused(usize),
    Detached(usize),
    Dropped(usize),
    /// 一次 `document.adoptedStyleSheets = [...]`
    Adopted(Vec<usize>),
}

/// 一张表当前的样子。表被 `Drop` 之后这份记录仍然留着，好让测试断言
/// 「退休的表内容还在」这类事情。
#[derive(Debug, Clone, Default)]
pub(crate) struct SheetLog {
    pub content: String,
    pub rules: Vec<String>,
    pub detached: bool,
    pub dropped: bool,
}

/// 让测试指定下一张表建出来是什么样。
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Knobs {
    /// `create()` 返回 `None`——两条构造路都走不通
    pub create_fails: bool,
    /// 建出来的表模拟 `<style>` 兜底：不参与 adoptedStyleSheets，也不吃增量追加
    pub tag_fallback: bool,
    /// `append_rules` 一律被拒，用来走「退回整表重建」那条路
    pub append_fails: bool,
}

thread_local! {
    static EVENTS: RefCell<Vec<SheetEvent>> = const { RefCell::new(Vec::new()) };
    static LOGS: RefCell<BTreeMap<usize, SheetLog>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_ID: Cell<usize> = const { Cell::new(0) };
    static KNOBS: Cell<Knobs> = const { Cell::new(Knobs {
        create_fails: false,
        tag_fallback: false,
        append_fails: false,
    }) };
    /// 当前文档上挂着的那一批表
    static ADOPTED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

// `Drop` 会在线程退出时被 TLS 析构器调用，那时别的 TLS 可能已经没了：一律
// `try_with`，记不上就算了，绝不能 panic（TLS 析构器里 panic 直接 abort）。
fn push_event(e: SheetEvent) {
    let _ = EVENTS.try_with(|ev| ev.borrow_mut().push(e));
}

fn with_log(id: usize, f: impl FnOnce(&mut SheetLog)) {
    let _ = LOGS.try_with(|l| f(l.borrow_mut().entry(id).or_default()));
}

/// 一张假的样式表。
pub(crate) struct FakeSheet {
    id: usize,
    /// 建表时的旋钮快照——建好之后再改旋钮不影响已有的表
    tag_fallback: bool,
    append_fails: bool,
}

impl SheetBackend for FakeSheet {
    type Handle = usize;

    fn create() -> Option<Self> {
        let knobs = KNOBS.with(Cell::get);
        if knobs.create_fails {
            return None;
        }
        let id = NEXT_ID.with(|n| {
            let id = n.get();
            n.set(id + 1);
            id
        });
        with_log(id, |_| {});
        push_event(SheetEvent::Created(id));
        Some(Self {
            id,
            tag_fallback: knobs.tag_fallback,
            append_fails: knobs.append_fails,
        })
    }

    fn replace(&self, css: &str) {
        with_log(self.id, |log| {
            log.content = css.to_string();
            log.rules.clear();
        });
        push_event(SheetEvent::Replaced(self.id, css.to_string()));
    }

    fn append_rules(&self, rules: &[&str]) -> bool {
        if self.tag_fallback || self.append_fails {
            push_event(SheetEvent::AppendRefused(self.id));
            return false;
        }
        let owned: Vec<String> = rules.iter().map(|r| (*r).to_string()).collect();
        with_log(self.id, |log| {
            for rule in &owned {
                log.content.push_str(rule);
                log.rules.push(rule.clone());
            }
        });
        push_event(SheetEvent::Appended(self.id, owned));
        true
    }

    fn adopted(&self) -> Option<usize> {
        if self.tag_fallback {
            None
        } else {
            Some(self.id)
        }
    }

    fn detach(&self) {
        if !self.tag_fallback {
            return;
        }
        with_log(self.id, |log| log.detached = true);
        push_event(SheetEvent::Detached(self.id));
    }
}

impl Drop for FakeSheet {
    fn drop(&mut self) {
        with_log(self.id, |log| log.dropped = true);
        push_event(SheetEvent::Dropped(self.id));
    }
}

/// 假的文档。
pub(crate) struct FakeDocument;

impl DocumentBackend<usize> for FakeDocument {
    fn set_adopted(sheets: &[usize]) {
        let _ = ADOPTED.try_with(|a| *a.borrow_mut() = sheets.to_vec());
        push_event(SheetEvent::Adopted(sheets.to_vec()));
    }
}

// ---- 测试侧的观察窗 ----

/// 文档上当前挂着的表。
#[cfg(test)]
pub(crate) fn adopted_now() -> Vec<usize> {
    ADOPTED.with(|a| a.borrow().clone())
}

/// 每一次 `set_adopted` 的快照，用来断言时序。
#[cfg(test)]
pub(crate) fn adopted_history() -> Vec<Vec<usize>> {
    EVENTS.with(|ev| {
        ev.borrow()
            .iter()
            .filter_map(|e| match e {
                SheetEvent::Adopted(ids) => Some(ids.clone()),
                _ => None,
            })
            .collect()
    })
}

#[cfg(test)]
pub(crate) fn events() -> Vec<SheetEvent> {
    EVENTS.with(|ev| ev.borrow().clone())
}

/// 某张表现在的样子（表已经被 `Drop` 也照样能读到）。
#[cfg(test)]
pub(crate) fn sheet_log(id: usize) -> SheetLog {
    LOGS.with(|l| l.borrow().get(&id).cloned().unwrap_or_default())
}

#[cfg(test)]
pub(crate) fn set_knobs(knobs: Knobs) {
    KNOBS.with(|k| k.set(knobs));
}

#[cfg(test)]
pub(crate) fn reset() {
    EVENTS.with(|ev| ev.borrow_mut().clear());
    LOGS.with(|l| l.borrow_mut().clear());
    ADOPTED.with(|a| a.borrow_mut().clear());
    NEXT_ID.with(|n| n.set(0));
    KNOBS.with(|k| k.set(Knobs::default()));
}
