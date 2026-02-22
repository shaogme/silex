use silex_dom::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
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
}

impl StaticStyleRegistry {
    pub(crate) fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        thread_local! {
            static INSTANCE: RefCell<StaticStyleRegistry> = RefCell::new(StaticStyleRegistry {
                injected_ids: HashSet::new(),
                shared_sheet: None,
            });
        }
        INSTANCE.with(|i| f(&mut i.borrow_mut()))
    }

    pub fn inject(&mut self, id: &str, content: &str) {
        if self.injected_ids.contains(id) {
            return;
        }
        self.injected_ids.insert(id.to_string());

        if let Some(sheet) = &self.shared_sheet {
            // Incremental injection: insert each rule one by one to avoid full re-parsing
            let rules = split_rules(content);
            for rule in rules {
                if let Ok(rule_list) = sheet.css_rules() {
                    let _ = sheet.insert_rule_with_index(rule, rule_list.length());
                }
            }
        } else {
            // Initialize sheet
            let sheet = CssStyleSheet::new().expect("Failed to create CssStyleSheet");
            let _ = sheet.replace_sync(content);

            // Register as the static sheet in the document registry
            DOCUMENT_REGISTRY.with(|dr| dr.borrow_mut().set_static_sheet(sheet.clone()));

            self.shared_sheet = Some(sheet);
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
            b'}' if in_quote.is_none() => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        let rule = css[start..i + 1].trim();
                        if !rule.is_empty() {
                            rules.push(rule);
                        }
                        start = i + 1;
                    }
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
    StaticStyleRegistry::with(|r| r.inject(id, content));
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

        wasm_bindgen_futures::spawn_local(async {
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

        let arr: js_sys::Array = new_list.into_iter().collect();
        doc.set_adopted_style_sheets(&arr);

        // Record the IDs for future comparison
        self.last_sync_ids = current_ids;
    }
}
