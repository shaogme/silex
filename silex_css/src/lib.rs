pub mod builder;
pub mod registry;
pub mod theme;
pub mod types;

pub mod prelude {
    pub use crate::builder::{Style, sty};
    pub use crate::theme::{ThemeVariables, set_global_theme, theme_variables, use_theme};
    pub use crate::types::*;
}

use silex_core::prelude::*;
use silex_dom::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::rc::{Rc, Weak};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CssStyleSheet, Element};

/// A global registry for static styles to avoid duplicated styles.
/// It merges all static styles into a single shared Constructable StyleSheet.
pub struct StaticStyleRegistry {
    /// Set of already injected style IDs.
    injected_ids: HashSet<String>,
    /// The shared stylesheet for all static styles.
    shared_sheet: Option<CssStyleSheet>,
}

impl StaticStyleRegistry {
    fn with<R>(f: impl FnOnce(&mut Self) -> R) -> R {
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
fn split_rules(css: &str) -> Vec<&str> {
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

pub type CssVariableGetter = Rc<dyn Fn() -> String>;

/// Manages an injected stylesheet uniquely for a component instance.
struct DynamicStyleState {
    id: String,
    sheet: CssStyleSheet,
}

impl Drop for DynamicStyleState {
    fn drop(&mut self) {
        // 1. Remove from document stylesheets
        DOCUMENT_REGISTRY.with(|dr| {
            if let Ok(mut dr) = dr.try_borrow_mut() {
                dr.remove_sheet(&self.sheet);
            }
        });
        // 2. Remove from registry map
        DYNAMIC_STYLE_REGISTRY.with(|reg| {
            if let Ok(mut reg) = reg.try_borrow_mut() {
                reg.remove(&self.id);
            }
        });
    }
}

const CACHE_LIMIT: usize = 128;

thread_local! {
    static DYNAMIC_STYLE_REGISTRY: RefCell<HashMap<String, Weak<DynamicStyleState>>> = RefCell::new(HashMap::new());
    static RETIRED_STYLES: RefCell<VecDeque<Rc<DynamicStyleState>>> = const { RefCell::new(VecDeque::new()) };
    static DOCUMENT_REGISTRY: RefCell<DocumentStyleRegistry> = RefCell::new(DocumentStyleRegistry::new());
}

/// Registry to manage the list of adopted stylesheets in the document.
/// This is the single source of truth for document.adoptedStyleSheets.
struct DocumentStyleRegistry {
    static_sheet: Option<CssStyleSheet>,
    dynamic_sheets: Vec<CssStyleSheet>,
    is_pending: bool,
}

impl DocumentStyleRegistry {
    fn new() -> Self {
        Self {
            static_sheet: None,
            dynamic_sheets: Vec::new(),
            is_pending: false,
        }
    }

    fn set_static_sheet(&mut self, sheet: CssStyleSheet) {
        self.static_sheet = Some(sheet);
        self.sync();
    }

    fn add_sheet(&mut self, sheet: CssStyleSheet) {
        self.dynamic_sheets.push(sheet);
        self.sync();
    }

    fn remove_sheet(&mut self, sheet: &CssStyleSheet) {
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
                dr.borrow_mut().perform_sync();
            });
        });
    }

    fn perform_sync(&mut self) {
        self.is_pending = false;

        let doc = document();

        let mut new_list: Vec<JsValue> =
            Vec::with_capacity((self.static_sheet.is_some() as usize) + self.dynamic_sheets.len());

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
    }
}

/// Manages an injected <style> block uniquely for a component instance.
/// It cleans up the tag when dropped, preventing CSSOM leaks.
pub struct DynamicStyleManager {
    state: Option<Rc<DynamicStyleState>>,
}

impl Default for DynamicStyleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicStyleManager {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub fn new_with_id(id: &str) -> Self {
        let mut mgr = Self::new();
        mgr.update(id, "");
        mgr
    }

    /// Moves the current style state to the retired cache if it's the last active reference.
    fn take_and_retire(&mut self) {
        if let Some(state) = self.state.take() {
            // If strong_count is 1, it means this manager was the only one holding the style.
            // We move it to RETIRED_STYLES to keep it alive for potential reuse.
            if Rc::strong_count(&state) == 1 {
                RETIRED_STYLES.with(|retired| {
                    let mut r = retired.borrow_mut();
                    r.push_back(state);
                    if r.len() > CACHE_LIMIT {
                        // This will drop the oldest retired state, potentially triggering DynamicStyleState::drop
                        r.pop_front();
                    }
                });
            }
        }
    }

    pub fn update(&mut self, id: &str, content: &str) {
        if let Some(state) = &self.state
            && state.id == id {
                return;
            }

        let new_state = DYNAMIC_STYLE_REGISTRY.with(|registry| {
            let mut reg = registry.borrow_mut();

            // Try to upgrade from registry (which holds weak references)
            if let Some(weak) = reg.get(id)
                && let Some(state) = weak.upgrade() {
                    // It's still alive (either in use or in retirement)
                    // If it was in retirement, we should remove it from the retired list
                    RETIRED_STYLES.with(|retired| {
                        let mut r = retired.borrow_mut();
                        if let Some(pos) = r.iter().position(|s| s.id == id) {
                            r.remove(pos);
                        }
                    });
                    return state;
                }

            // Not found or was dropped, create a new one
            let sheet = CssStyleSheet::new().expect("Failed to create CssStyleSheet");
            let _ = sheet.replace_sync(content);
            DOCUMENT_REGISTRY.with(|dr| dr.borrow_mut().add_sheet(sheet.clone()));

            let state = Rc::new(DynamicStyleState {
                id: id.to_string(),
                sheet,
            });
            reg.insert(id.to_string(), Rc::downgrade(&state));
            state
        });

        self.take_and_retire();
        self.state = Some(new_state);
    }
}

impl Drop for DynamicStyleManager {
    fn drop(&mut self) {
        self.take_and_retire();
    }
}

/// A structure representing a dynamic CSS class with reactive variables and dynamic rules.
/// Generated by the `css!` macro when dynamic interpolation `$(...)` is used.
#[derive(Clone)]
pub struct DynamicCss {
    /// The generated class name (e.g., "slx-1234abcd")
    pub class_name: &'static str,
    /// A list of (css_variable_name, value_getter) pairs.
    /// These are applied as inline styles to the element.
    pub vars: Vec<(&'static str, CssVariableGetter)>,
    /// A list of (css_template, list of value getters) pairs for dynamic selector blocks.
    pub rules: Vec<(&'static str, Vec<CssVariableGetter>)>,
}

impl ApplyToDom for DynamicCss {
    fn apply(self, el: &Element, target: ApplyTarget) {
        // 1. Apply class name (as normal string class)
        // This ensures the element gets the static CSS rules.
        self.class_name.apply(el, target);

        // 2. Apply dynamic variables (always as inline styles)
        // Optimization: Coalesce all variable updates into a single effect
        if !self.vars.is_empty() {
            let el = el.clone();
            let vars = self.vars;
            ::silex_core::prelude::Effect::new(move |prev_values: Option<Vec<String>>| {
                use ::wasm_bindgen::JsCast;
                if let Some(style) = el
                    .dyn_ref::<::web_sys::HtmlElement>()
                    .map(|e| e.style())
                    .or_else(|| el.dyn_ref::<::web_sys::SvgElement>().map(|e| e.style()))
                {
                    let mut current_vals = Vec::with_capacity(vars.len());
                    for (i, (name, getter)) in vars.iter().enumerate() {
                        let value = getter();
                        if prev_values.as_ref().and_then(|v| v.get(i)) != Some(&value) {
                            let _ = style.set_property(name, &value);
                        }
                        current_vals.push(value);
                    }
                    return current_vals;
                }
                Vec::new()
            });
        }

        // 3. Apply isolated component dynamic rules
        // Optimization: Each rule now gets its own effect and hashing.
        // This allows styles to be reused if only some properties change, or if multiple components share a rule.
        for (template, getters) in self.rules {
            let manager = Rc::new(RefCell::new(Some(DynamicStyleManager::new())));
            let manager_cleanup = manager.clone();
            on_cleanup(move || {
                if let Ok(mut opt_mgr) = manager_cleanup.try_borrow_mut() {
                    let _ = opt_mgr.take();
                }
            });

            let el_clone = el.clone();
            let base_class = self.class_name;

            Effect::new(move |prev: Option<(Vec<String>, String)>| {
                // 1. Get current values and compare with previous
                let current_vals: Vec<String> = getters.iter().map(|g| g()).collect();
                if let Some((old_vals, _)) = &prev
                    && current_vals == *old_vals
                {
                    return prev.unwrap();
                }
                // Values changed, we will proceed to re-hash and update
                // Note: old_class is still applied to el_clone, it will be removed below if name changes

                // 2. Compute new resolve rule and hash (Only when values changed)
                let mut resolved_rule = template.to_string();
                for val in &current_vals {
                    if let Some(pos) = resolved_rule.find("{}") {
                        resolved_rule.replace_range(pos..pos + 2, val);
                    }
                }

                let hash_val = silex_hash::css::hash_one((
                    b"silex-dyn-v3",
                    silex_hash::css::Normalized(template),
                    silex_hash::css::Normalized(&resolved_rule),
                ));
                let mut hash_buf = [0u8; 13];
                let hash_str = silex_hash::css::encode_base36(hash_val, &mut hash_buf);
                let dyn_class = format!("{}-d{}", base_class, hash_str);

                // 3. Update DOM and Registry if name changed
                let prev_class = prev.as_ref().map(|(_, c)| c);
                if Some(&dyn_class) != prev_class {
                    if let Some(old_class) = prev_class {
                        let _ = el_clone.class_list().remove_1(old_class);
                    }
                    let _ = el_clone.class_list().add_1(&dyn_class);

                    let dot_base = format!(".{}", base_class);
                    let dot_dyn = format!(".{}", dyn_class);
                    let rule_with_dyn_class = resolved_rule.replace(&dot_base, &dot_dyn);

                    if let Ok(mut opt) = manager.try_borrow_mut()
                        && let Some(mgr) = opt.as_mut()
                    {
                        mgr.update(&dyn_class, &rule_with_dyn_class);
                    }
                }

                (current_vals, dyn_class)
            });
        }
    }
}

// Allow passing DynamicCss directly to .class() or .attr()
impl IntoStorable for DynamicCss {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

/// Helper function to create a reactive string getter from any signal-like value.
/// Used by the css! macro to handle $(...) interpolation.
pub fn make_dynamic_val_for<P, S>(source: S) -> Rc<dyn Fn() -> String>
where
    S: IntoSignal,
    S::Value: Clone + Sized + types::ValidFor<P> + Display,
    S::Signal: Get + 'static,
    <S::Signal as With>::Value: Display,
{
    let signal = source.into_signal();
    Rc::new(move || format!("{}", signal.get()))
}
