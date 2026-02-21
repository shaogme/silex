pub mod builder;
pub mod registry;
pub mod theme;
pub mod types;

pub mod prelude {
    pub use crate::builder::{Style, sty};
    pub use crate::theme::{ThemeVariables, set_global_theme, theme_variables, use_theme};
    pub use crate::types::*;
}

use silex_core::reactivity::{Effect, on_cleanup};
use silex_core::traits::{Get, IntoSignal, With};
use silex_dom::attribute::{ApplyTarget, ApplyToDom, IntoStorable};
use silex_dom::document;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque, hash_map::DefaultHasher};
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node, SvgElement};

/// Injects a CSS string into the document head with a unique ID.
/// This function is idempotent: if a style with the given ID already exists, it does nothing.
///
/// # Arguments
///
/// * `id` - A unique identifier for the style block (e.g. "style-slx-123456").
/// * `content` - The CSS content to inject.
pub fn inject_style(id: &str, content: &str) {
    let doc = document();

    // Check if style already exists to avoid duplication
    if doc.get_element_by_id(id).is_some() {
        return;
    }

    let head = doc.head().expect("No <head> element found in document");

    // Create <style> element
    let style_el = doc
        .create_element("style")
        .expect("Failed to create style element");

    // Set ID and content
    style_el.set_id(id);
    style_el.set_inner_html(content);

    // Append to head
    let style_node: Node = style_el.unchecked_into();
    head.append_child(&style_node)
        .expect("Failed to append style to head");
}

/// Updates the content of an existing style block by ID.
/// If it doesn't exist, it will be created via `inject_style`.
pub fn update_style(id: &str, content: &str) {
    let doc = document();
    if let Some(el) = doc.get_element_by_id(id) {
        el.set_inner_html(content);
    } else {
        inject_style(id, content);
    }
}

/// Applies a CSS variable string (e.g. "--var: val; --var2: val2;") to the root element (:root).
pub fn apply_vars_to_root(vars: &str) {
    let doc = document();
    if let Some(root) = doc.document_element()
        && root.dyn_ref::<HtmlElement>().is_some()
    {
        let css = format!(":root {{ {} }}", vars);
        update_style("silex-theme-root", &css);
    }
}

pub type CssVariableGetter = Rc<dyn Fn() -> String>;

/// Manages an injected <style> block uniquely for a component instance.
/// It cleans up the tag when dropped, preventing CSSOM leaks.
struct DynamicStyleState {
    ref_count: usize,
    style_el: Element,
}

const CACHE_LIMIT: usize = 128;

thread_local! {
    static DYNAMIC_STYLE_REGISTRY: RefCell<HashMap<String, Rc<RefCell<DynamicStyleState>>>> = RefCell::new(HashMap::new());
    static RETIRED_STYLES: RefCell<VecDeque<String>> = const { RefCell::new(VecDeque::new()) };
}

/// Manages an injected <style> block uniquely for a component instance.
/// It cleans up the tag when dropped, preventing CSSOM leaks.
pub struct DynamicStyleManager {
    id: Option<String>,
}

impl Default for DynamicStyleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicStyleManager {
    pub fn new() -> Self {
        Self { id: None }
    }

    pub fn new_with_id(id: &str) -> Self {
        let mut mgr = Self::new();
        mgr.update(id, "");
        mgr
    }

    pub fn update(&mut self, id: &str, content: &str) {
        if self.id.as_deref() == Some(id) {
            return;
        }

        DYNAMIC_STYLE_REGISTRY.with(|registry| {
            let mut reg = registry.borrow_mut();

            if let Some(state_rc) = reg.get(id).cloned() {
                let mut state = state_rc.borrow_mut();
                if state.ref_count == 0 {
                    // Remove from retired list if it was there
                    RETIRED_STYLES.with(|retired| {
                        let mut r = retired.borrow_mut();
                        if let Some(pos) = r.iter().position(|x| x == id) {
                            r.remove(pos);
                        }
                    });
                }
                state.ref_count += 1;
            } else {
                let doc = document();
                let head = doc.head().expect("No <head> element found in document");

                let style_el = doc
                    .create_element("style")
                    .expect("Failed to create style element");

                style_el.set_id(id);
                if !content.is_empty() {
                    style_el.set_inner_html(content);
                }

                let style_node: Node = style_el.clone().unchecked_into();
                head.append_child(&style_node)
                    .expect("Failed to append style to head");

                reg.insert(
                    id.to_string(),
                    Rc::new(RefCell::new(DynamicStyleState {
                        ref_count: 1,
                        style_el,
                    })),
                );
            }
        });

        if let Some(old_id) = self.id.take() {
            Self::release(&old_id);
        }
        self.id = Some(id.to_string());
    }

    fn release(id: &str) {
        let to_be_removed = DYNAMIC_STYLE_REGISTRY.with(|registry| {
            let mut reg = registry.borrow_mut();
            if let Some(state_rc) = reg.get(id).cloned() {
                let mut state = state_rc.borrow_mut();
                state.ref_count -= 1;
                if state.ref_count == 0 {
                    let id_str = id.to_string();
                    return RETIRED_STYLES.with(|retired| {
                        let mut r = retired.borrow_mut();
                        r.push_back(id_str);

                        if r.len() > CACHE_LIMIT
                            && let Some(to_remove_id) = r.pop_front()
                            && let Some(st_rc) = reg.remove(&to_remove_id)
                        {
                            return Some(st_rc);
                        }
                        None
                    });
                }
            }
            None
        });

        if let Some(st) = to_be_removed {
            st.borrow().style_el.remove();
        }
    }
}

impl Drop for DynamicStyleManager {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            Self::release(id);
        }
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
            Effect::new(move |_| {
                if let Some(style) = el
                    .dyn_ref::<HtmlElement>()
                    .map(|e| e.style())
                    .or_else(|| el.dyn_ref::<SvgElement>().map(|e| e.style()))
                {
                    for (name, getter) in &vars {
                        let value = getter();
                        let _ = style.set_property(name, &value);
                    }
                }
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

            Effect::new(move |prev_class: Option<String>| {
                let mut hasher = DefaultHasher::new();
                Hash::hash(b"silex-dyn-salt-css-v2", &mut hasher);
                Hash::hash(template, &mut hasher);

                let mut resolved_rule = template.to_string();
                for getter in &getters {
                    let val = getter();
                    if let Some(pos) = resolved_rule.find("{}") {
                        resolved_rule.replace_range(pos..pos + 2, &val);
                    }
                }

                Hash::hash(&resolved_rule, &mut hasher);
                let hash_val = hasher.finish();
                let dyn_class = format!("{}-dyn-{:x}", base_class, hash_val);

                if Some(&dyn_class) != prev_class.as_ref() {
                    if let Some(old_class) = &prev_class {
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

                dyn_class
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
