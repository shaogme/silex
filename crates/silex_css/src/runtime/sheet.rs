//! 浏览器后端：一张样式表的载体。
//!
//! 首选构造式样式表（`new CSSStyleSheet()` + `document.adoptedStyleSheets`）；
//! 环境不支持时退回一个挂在 `<head>` 里的 `<style>` 元素。
//!
//! 此前两处构造点的策略并不一致：静态表 `if let Ok(sheet)` 优雅降级——降级之后
//! 整段 CSS 直接消失，既没有 `<style>` 兜底也没有任何提示；动态表则是
//! `expect("Failed to create CssStyleSheet")` 直接 panic。同一个失败原因，
//! 一处静默丢样式、一处炸掉整个应用。
//!
//! 这个文件只在 wasm 目标上编译；它上面那层状态机不在这里，见 `backend.rs`。

use crate::runtime::{
    backend::{DocumentBackend, SheetBackend},
    platform::report,
};
use js_sys::Array;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{CssStyleSheet, Document, HtmlStyleElement};

fn document() -> Document {
    web_sys::window()
        .expect("CSS browser backend requires a window")
        .document()
        .expect("CSS browser backend requires a document")
}

/// 一张样式表。
pub(crate) enum Sheet {
    /// 构造式样式表，参与 `document.adoptedStyleSheets`
    #[cfg_attr(feature = "test-style-fallback", allow(dead_code))]
    Constructed(CssStyleSheet),
    /// `<style>` 兜底，直接活在 `<head>` 里，不参与 adoptedStyleSheets
    Tag(HtmlStyleElement),
}

impl Sheet {
    fn new_tag() -> Option<HtmlStyleElement> {
        let doc = document();
        let el = doc.create_element("style").ok()?;
        let head = doc.head()?;
        head.append_child(&el).ok()?;
        el.dyn_into::<HtmlStyleElement>().ok()
    }
}

impl SheetBackend for Sheet {
    type Handle = CssStyleSheet;

    fn create() -> Option<Self> {
        #[cfg(feature = "test-style-fallback")]
        {
            report("测试强制使用 <style> 兜底");
            Self::new_tag().map(Self::Tag)
        }

        #[cfg(not(feature = "test-style-fallback"))]
        {
            if let Ok(sheet) = CssStyleSheet::new() {
                return Some(Self::Constructed(sheet));
            }
            report("new CSSStyleSheet() 不可用，退回 <style> 兜底");
            Self::new_tag().map(Self::Tag)
        }
    }

    fn replace(&self, css: &str) -> bool {
        match self {
            Self::Constructed(s) => {
                if s.replace_sync(css).is_err() {
                    report("replaceSync 失败，本次样式未生效");
                    return false;
                }
                true
            }
            Self::Tag(el) => {
                el.set_text_content(Some(css));
                true
            }
        }
    }

    fn append_rules(&self, rules: &[&str]) -> bool {
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

    fn attach(&self) -> bool {
        match self {
            Self::Constructed(_) => true,
            Self::Tag(el) => {
                if el.parent_node().is_some() {
                    return true;
                }
                let Some(head) = document().head() else {
                    return false;
                };
                head.append_child(el).is_ok()
            }
        }
    }

    fn adopted(&self) -> Option<CssStyleSheet> {
        match self {
            Self::Constructed(s) => Some(s.clone()),
            Self::Tag(_) => None,
        }
    }

    fn detach(&self) {
        if let Self::Tag(el) = self {
            el.remove();
        }
    }
}

/// 真正的文档。
pub(crate) struct WebDocument;

impl DocumentBackend<CssStyleSheet> for WebDocument {
    fn set_adopted(sheets: &[CssStyleSheet]) {
        let arr: Array = sheets.iter().map(|s| JsValue::from(s.clone())).collect();
        document().set_adopted_style_sheets(&arr);
    }
}
