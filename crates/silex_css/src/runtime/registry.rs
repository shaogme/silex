use js_sys::Array;
use silex_dom::prelude::*;
use std::{cell::RefCell, collections::HashSet};
use wasm_bindgen::{JsCast, prelude::*};
use wasm_bindgen_futures::spawn_local;
use web_sys::CssStyleSheet;

thread_local! {
    pub(crate) static DOCUMENT_REGISTRY: RefCell<DocumentStyleRegistry> = RefCell::new(DocumentStyleRegistry::new());
}

/// A global registry for static styles to avoid duplicated styles.
/// It merges all static styles into a single shared Constructable StyleSheet.
pub struct StaticStyleRegistry {
    /// Set of already injected style IDs.
    injected_ids: HashSet<String>,
    /// The shared stylesheet for all static styles.
    shared_sheet: Option<CssStyleSheet>,
    /// Queue of pending rules awaiting microtask flush.
    pending_rules: Vec<String>,
    /// Whether a microtask flush has already been scheduled.
    is_flush_pending: bool,
}

impl StaticStyleRegistry {
    pub(crate) fn with<R>(f: impl FnOnce(&mut Self) -> Option<R>) -> Option<R> {
        thread_local! {
            static INSTANCE: RefCell<StaticStyleRegistry> = RefCell::new(StaticStyleRegistry {
                injected_ids: HashSet::new(),
                shared_sheet: None,
                pending_rules: Vec::new(),
                is_flush_pending: false,
            });
        }
        INSTANCE.with(|i| {
            if let Ok(mut reg) = i.try_borrow_mut() {
                f(&mut reg)
            } else {
                None
            }
        })
    }

    pub fn inject(&mut self, id: &str, content: &str) {
        if self.injected_ids.contains(id) {
            return;
        }
        self.injected_ids.insert(id.to_string());

        let rules = split_rules(content);
        for rule in rules {
            self.pending_rules.push(rule.to_string());
        }

        if self.shared_sheet.is_none() {
            if let Ok(sheet) = CssStyleSheet::new() {
                let mut init_content = String::from("@layer base, components, utilities;\n");
                for r in self.pending_rules.drain(..) {
                    init_content.push_str(&r);
                    init_content.push('\n');
                }
                let _ = sheet.replace_sync(&init_content);

                DOCUMENT_REGISTRY.with(|dr| {
                    if let Ok(mut dr) = dr.try_borrow_mut() {
                        dr.set_static_sheet(sheet.clone());
                    }
                });

                self.shared_sheet = Some(sheet);
                return;
            }
        }

        self.schedule_flush();
    }

    fn schedule_flush(&mut self) {
        if self.is_flush_pending || self.pending_rules.is_empty() {
            return;
        }
        self.is_flush_pending = true;

        spawn_local(async {
            StaticStyleRegistry::with(|r| {
                r.flush();
                Some(())
            });
        });
    }

    pub fn flush(&mut self) {
        self.is_flush_pending = false;
        if self.pending_rules.is_empty() {
            return;
        }

        if let Some(sheet) = &self.shared_sheet {
            for rule in self.pending_rules.drain(..) {
                if let Ok(rule_list) = sheet.css_rules() {
                    let _ = sheet.insert_rule_with_index(&rule, rule_list.length());
                }
            }
        } else {
            self.pending_rules.clear();
        }
    }
}

/// Helper to split a CSS string into top-level rules.
/// This is necessary because insert_rule only accepts a single rule.
pub fn split_rules(css: &str) -> Vec<&str> {
    let mut rules = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let mut in_quote = None;
    let bytes = css.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 1;
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
    StaticStyleRegistry::with(|r| {
        r.inject(id, content);
        Some(())
    });
}

/// Registry to manage the list of adopted stylesheets in the document.
/// This is the single source of truth for document.adoptedStyleSheets.
pub(crate) struct DocumentStyleRegistry {
    static_sheet: Option<CssStyleSheet>,
    dynamic_sheets: Vec<CssStyleSheet>,
    /// Tracks the identity (pointer-level) of the last synced list of sheets
    /// to avoid redundant `set_adopted_style_sheets` calls.
    last_sync_ids: Vec<usize>,
    is_pending: bool,
}

impl DocumentStyleRegistry {
    fn new() -> Self {
        Self {
            static_sheet: None,
            dynamic_sheets: Vec::new(),
            last_sync_ids: Vec::new(),
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

    fn sync(&mut self) {
        if self.is_pending {
            return;
        }

        self.is_pending = true;

        spawn_local(async {
            DOCUMENT_REGISTRY.with(|dr| {
                if let Ok(mut dr) = dr.try_borrow_mut() {
                    dr.perform_sync();
                }
            });
        });
    }

    fn perform_sync(&mut self) {
        self.is_pending = false;

        let num_sheets = (self.static_sheet.is_some() as usize) + self.dynamic_sheets.len();
        let mut current_ids = Vec::with_capacity(num_sheets);

        if let Some(sheet) = &self.static_sheet {
            current_ids.push(sheet.unchecked_ref::<JsValue>() as *const _ as usize);
        }
        for sheet in &self.dynamic_sheets {
            current_ids.push(sheet.unchecked_ref::<JsValue>() as *const _ as usize);
        }

        // Optimization: If the sheet list hasn't changed at the identity level,
        // we skip the browser-side adoptedStyleSheets update completely.
        if self.last_sync_ids == current_ids {
            return;
        }

        let doc = document();
        let mut new_list: Vec<JsValue> = Vec::with_capacity(num_sheets);

        // 1. Static sheet always comes first
        if let Some(sheet) = &self.static_sheet {
            new_list.push(sheet.clone().unchecked_into());
        }

        // 2. Add dynamic sheets
        for sheet in &self.dynamic_sheets {
            new_list.push(sheet.clone().unchecked_into());
        }

        let arr: Array = new_list.into_iter().collect();
        doc.set_adopted_style_sheets(&arr);

        // Record the IDs for future comparison
        self.last_sync_ids = current_ids;
    }
}
