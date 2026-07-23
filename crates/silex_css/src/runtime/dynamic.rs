use crate::{runtime::registry::DOCUMENT_REGISTRY, types};
use silex_core::{prelude::*, traits::RxGet};
use silex_dom::prelude::*;
use silex_hash::css::{Normalized, encode_base36, hash_one};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    fmt::Display,
    rc::{Rc, Weak},
};
use wasm_bindgen::JsCast;
use web_sys::{CssStyleSheet, Element, HtmlElement, SvgElement};

pub type CssVariableGetter = Rx<String>;

const CACHE_LIMIT: usize = 128;

thread_local! {
    static DYNAMIC_STYLE_REGISTRY: RefCell<HashMap<String, Weak<DynamicStyleState>>> = RefCell::new(HashMap::new());
    static RETIRED_STYLES: RefCell<VecDeque<Rc<DynamicStyleState>>> = const { RefCell::new(VecDeque::new()) };
}

/// Manages an injected stylesheet uniquely for a component instance.
pub(crate) struct DynamicStyleState {
    pub id: String,
    pub sheet: CssStyleSheet,
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
            && state.id == id
        {
            let _ = state.sheet.replace_sync(content);
            return;
        }

        let new_state = DYNAMIC_STYLE_REGISTRY.with(|registry| {
            let mut reg = registry.borrow_mut();

            if let Some(weak) = reg.get(id)
                && let Some(state) = weak.upgrade()
            {
                RETIRED_STYLES.with(|retired| {
                    let mut r = retired.borrow_mut();
                    if let Some(pos) = r.iter().position(|s| s.id == id) {
                        r.remove(pos);
                    }
                });
                let _ = state.sheet.replace_sync(content);
                return state;
            }
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
#[derive(Clone)]
pub struct DynamicCss {
    pub class_name: &'static str,
    pub vars: Vec<(&'static str, CssVariableGetter)>,
    pub rules: Vec<(&'static str, Vec<CssVariableGetter>)>,
}

impl DynamicCss {
    pub fn new(class_name: &'static str) -> Self {
        Self {
            class_name,
            vars: Vec::new(),
            rules: Vec::new(),
        }
    }

    pub fn with_var<P, S>(mut self, var_name: &'static str, source: S) -> Self
    where
        P: types::CssProperty,
        S: IntoRx,
        S::Value: Clone + Sized + types::ValidFor<P> + Display + 'static,
        S::RxType: RxGet<Value = S::Value> + 'static,
    {
        self.vars
            .push((var_name, make_property_val::<P, S>(source)));
        self
    }

    pub fn with_rule(mut self, template: &'static str, exprs: Vec<CssVariableGetter>) -> Self {
        self.rules.push((template, exprs));
        self
    }
}

impl ApplyToDom for DynamicCss {
    fn apply(&self, el: &Element, _target: ApplyTarget) {
        // 1. Apply class name
        self.class_name.apply(el, ApplyTarget::Class);

        // 2. Apply inline variables with optimized Effect
        if !self.vars.is_empty() {
            let el = el.clone();
            let vars = self.vars.clone();
            Effect::new(move |prev_values: Option<Vec<String>>| {
                let Some(style) = el
                    .dyn_ref::<HtmlElement>()
                    .map(|e| e.style())
                    .or_else(|| el.dyn_ref::<SvgElement>().map(|e| e.style()))
                else {
                    return Vec::new();
                };

                let mut current_vals = Vec::with_capacity(vars.len());
                let mut changed = false;

                for (i, (_name, getter)) in vars.iter().enumerate() {
                    let val = getter.get();
                    if !changed && prev_values.as_ref().and_then(|v| v.get(i)) != Some(&val) {
                        changed = true;
                    }
                    current_vals.push(val);
                }

                if changed || prev_values.is_none() {
                    for (i, (name, val)) in vars.iter().zip(current_vals.iter()).enumerate() {
                        if prev_values.as_ref().and_then(|v| v.get(i)) != Some(val) {
                            let _ = style.set_property(name.0, val);
                        }
                    }
                }
                current_vals
            });
        }

        // 3. Apply isolated component dynamic rules
        for (template, getters) in self.rules.clone() {
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
                let current_vals: Vec<String> = getters.iter().map(|g| g.get()).collect();
                if let Some((old_vals, _)) = &prev
                    && current_vals == *old_vals
                {
                    return prev.unwrap();
                }

                let mut resolved_rule = String::with_capacity(
                    template.len() + current_vals.iter().map(|v| v.len()).sum::<usize>(),
                );
                let mut last_pos = 0;
                let mut vals_iter = current_vals.iter();

                while let Some(pos) = template[last_pos..].find("{}") {
                    if let Some(val) = vals_iter.next() {
                        let actual_pos = last_pos + pos;
                        resolved_rule.push_str(&template[last_pos..actual_pos]);
                        resolved_rule.push_str(val);
                        last_pos = actual_pos + 2;
                    } else {
                        break;
                    }
                }
                resolved_rule.push_str(&template[last_pos..]);

                let hash_val = hash_one((
                    b"silex-dyn-v3",
                    Normalized(template),
                    Normalized(&resolved_rule),
                ));
                let mut hash_buf = [0u8; 13];
                let hash_str = encode_base36(hash_val, &mut hash_buf);
                let dyn_class = format!("{}-d{}", base_class, hash_str);

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

impl IntoStorable for DynamicCss {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

pub fn make_property_val<P, S>(source: S) -> Rx<String>
where
    P: types::CssProperty,
    S: IntoRx,
    S::Value: Clone + Sized + types::ValidFor<P> + Display + 'static,
    S::RxType: RxGet<Value = S::Value> + 'static,
{
    let signal = source.into_rx();
    Rx::derive(Box::new(move || format!("{}", signal.get())))
}

pub fn make_dynamic_val_for<P, S>(source: S) -> Rx<String>
where
    P: types::CssProperty,
    S: IntoRx,
    S::Value: Clone + Sized + types::ValidFor<P> + Display + 'static,
    S::RxType: RxGet<Value = S::Value> + 'static,
{
    make_property_val::<P, S>(source)
}

/// Helper function to inject managed dynamic style with reactive variable replacements.
pub fn inject_managed_dynamic_style(
    style_id: impl Into<String>,
    template: String,
    replacements: Vec<(String, CssVariableGetter)>,
) {
    let manager = Rc::new(RefCell::new(Some(DynamicStyleManager::new())));
    let cleanup_mgr = manager.clone();
    on_cleanup(move || {
        if let Ok(mut opt) = cleanup_mgr.try_borrow_mut() {
            let _ = opt.take();
        }
    });

    let style_id_str = style_id.into();
    Effect::new(move |_| {
        let mut res = template.clone();
        for (pattern, getter) in &replacements {
            let val = getter.get();
            res = res.replace(pattern, &val);
        }
        if let Ok(mut opt) = manager.try_borrow_mut()
            && let Some(m) = opt.as_mut()
        {
            m.update(&style_id_str, &res);
        }
    });
}
