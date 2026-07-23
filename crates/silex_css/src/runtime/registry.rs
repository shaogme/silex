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
    /// All injected static CSS content chunks.
    all_chunks: Vec<String>,
    /// Whether there are new chunks awaiting microtask flush.
    has_pending_chunks: bool,
    /// Whether a microtask flush has already been scheduled.
    is_flush_pending: bool,
}

impl StaticStyleRegistry {
    pub(crate) fn with<R>(f: impl FnOnce(&mut Self) -> Option<R>) -> Option<R> {
        thread_local! {
            static INSTANCE: RefCell<StaticStyleRegistry> = RefCell::new(StaticStyleRegistry {
                injected_ids: HashSet::new(),
                shared_sheet: None,
                all_chunks: Vec::new(),
                has_pending_chunks: false,
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

        let trimmed = content.trim();
        if trimmed.is_empty() {
            return;
        }

        self.injected_ids.insert(id.to_string());
        self.all_chunks.push(trimmed.to_string());
        self.has_pending_chunks = true;

        if self.shared_sheet.is_none()
            && let Ok(sheet) = CssStyleSheet::new()
        {
            let init_content = self.build_full_content();
            let _ = sheet.replace_sync(&init_content);
            self.has_pending_chunks = false;

            DOCUMENT_REGISTRY.with(|dr| {
                if let Ok(mut dr) = dr.try_borrow_mut() {
                    dr.set_static_sheet(sheet.clone());
                }
            });

            self.shared_sheet = Some(sheet);
            return;
        }

        self.schedule_flush();
    }

    fn schedule_flush(&mut self) {
        if self.is_flush_pending || !self.has_pending_chunks {
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
        if !self.has_pending_chunks {
            return;
        }
        self.has_pending_chunks = false;

        if let Some(sheet) = &self.shared_sheet {
            let full_content = self.build_full_content();
            let _ = sheet.replace_sync(&full_content);
        }
    }

    /// Pre-allocates string capacity and builds the full CSS stylesheet content.
    fn build_full_content(&self) -> String {
        const LAYER_HEADER: &str = "@layer base, components, utilities;\n";
        let capacity =
            LAYER_HEADER.len() + self.all_chunks.iter().map(|c| c.len() + 1).sum::<usize>();
        let mut full_content = String::with_capacity(capacity);
        full_content.push_str(LAYER_HEADER);
        for chunk in &self.all_chunks {
            full_content.push_str(chunk);
            full_content.push('\n');
        }
        full_content
    }
}

/// Helper to split a CSS string into top-level rules.
/// Handles nested blocks (`{}`), strings, escapes, and CSS comments (`/* ... */`).
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
