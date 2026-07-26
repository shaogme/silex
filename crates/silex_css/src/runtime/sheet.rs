//! 一张样式表的载体，以及运行时问题的统一出口。
//!
//! 首选构造式样式表（`new CSSStyleSheet()` + `document.adoptedStyleSheets`）；
//! 环境不支持时退回一个挂在 `<head>` 里的 `<style>` 元素。
//!
//! 此前两处构造点的策略并不一致：静态表 `if let Ok(sheet)` 优雅降级——降级之后
//! 整段 CSS 直接消失，既没有 `<style>` 兜底也没有任何提示；动态表则是
//! `expect("Failed to create CssStyleSheet")` 直接 panic。同一个失败原因，
//! 一处静默丢样式、一处炸掉整个应用。

use silex_dom::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CssStyleSheet, HtmlStyleElement};

/// 运行时异常的统一出口。
///
/// 样式注入失败、清理失败这类问题此前一律 `let _ = …` 吞掉，症状是「样式莫名
/// 不生效」而没有任何线索。debug 构建下打到 `console.error`，release 下不产生
/// 任何代码。
#[inline]
pub(crate) fn report(_what: &str) {
    #[cfg(debug_assertions)]
    web_sys::console::error_1(&format!("[silex-css] {}", _what).into());
}

/// 一张样式表。
pub(crate) enum Sheet {
    /// 构造式样式表，参与 `document.adoptedStyleSheets`
    Constructed(CssStyleSheet),
    /// `<style>` 兜底，直接活在 `<head>` 里，不参与 adoptedStyleSheets
    Tag(HtmlStyleElement),
}

impl Sheet {
    /// 建一张新表。两条路都走不通时返回 `None`（此时调用方应当放弃，而不是
    /// 假装成功）。
    pub(crate) fn new() -> Option<Self> {
        if let Ok(sheet) = CssStyleSheet::new() {
            return Some(Self::Constructed(sheet));
        }
        report("new CSSStyleSheet() 不可用，退回 <style> 兜底");
        Self::new_tag().map(Self::Tag)
    }

    fn new_tag() -> Option<HtmlStyleElement> {
        let doc = document();
        let el = doc.create_element("style").ok()?;
        let head = doc.head()?;
        head.append_child(&el).ok()?;
        el.dyn_into::<HtmlStyleElement>().ok()
    }

    /// 整表替换。
    pub(crate) fn replace(&self, css: &str) {
        match self {
            Self::Constructed(s) => {
                if s.replace_sync(css).is_err() {
                    report("replaceSync 失败，本次样式未生效");
                }
            }
            Self::Tag(el) => el.set_text_content(Some(css)),
        }
    }

    /// 往表尾追加若干条**顶层规则**。
    ///
    /// 成功返回 `true`；`<style>` 兜底或 `insertRule` 抛错时返回 `false`，
    /// 由调用方退回整表替换。
    pub(crate) fn append_rules(&self, rules: &[&str]) -> bool {
        let Self::Constructed(sheet) = self else {
            return false;
        };
        for rule in rules {
            let Ok(list) = sheet.css_rules() else {
                report("读取 cssRules 失败，退回整表替换");
                return false;
            };
            if sheet.insert_rule_with_index(rule, list.length()).is_err() {
                report(&format!("insertRule 失败，退回整表替换：{rule}"));
                return false;
            }
        }
        true
    }

    /// 参与 `adoptedStyleSheets` 的那一张（`<style>` 兜底没有）。
    pub(crate) fn adopted(&self) -> Option<&CssStyleSheet> {
        match self {
            Self::Constructed(s) => Some(s),
            Self::Tag(_) => None,
        }
    }

    /// 把 `<style>` 兜底从文档里摘掉。构造式样式表由 `DocumentStyleRegistry`
    /// 负责摘除，这里不重复。
    pub(crate) fn detach(&self) {
        if let Self::Tag(el) = self {
            el.remove();
        }
    }
}
