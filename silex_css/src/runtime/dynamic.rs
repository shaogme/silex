use crate::runtime::registry::DOCUMENT_REGISTRY;
use crate::types;
use silex_core::prelude::*;
use silex_dom::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::rc::{Rc, Weak};
use wasm_bindgen::JsCast;
use web_sys::{CssStyleSheet, Element};

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
        if let Some(state) = &self.state {
            if state.id == id {
                return;
            }
        }

        let new_state = DYNAMIC_STYLE_REGISTRY.with(|registry| {
            let mut reg = registry.borrow_mut();

            if let Some(weak) = reg.get(id) {
                if let Some(state) = weak.upgrade() {
                    RETIRED_STYLES.with(|retired| {
                        let mut r = retired.borrow_mut();
                        if let Some(pos) = r.iter().position(|s| s.id == id) {
                            r.remove(pos);
                        }
                    });
                    return state;
                }
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

impl ApplyToDom for DynamicCss {
    fn apply(&self, el: &Element, target: ApplyTarget) {
        // 1. Apply class name
        self.class_name.apply(el, target);

        // 2. Apply inline variables with optimized Effect
        if !self.vars.is_empty() {
            let el = el.clone();
            let vars = self.vars.clone();
            Effect::new(move |prev_values: Option<Vec<String>>| {
                let Some(style) = el
                    .dyn_ref::<web_sys::HtmlElement>()
                    .map(|e| e.style())
                    .or_else(|| el.dyn_ref::<web_sys::SvgElement>().map(|e| e.style()))
                else { return Vec::new() };

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

                let hash_val = silex_hash::css::hash_one((
                    b"silex-dyn-v3",
                    silex_hash::css::Normalized(template),
                    silex_hash::css::Normalized(&resolved_rule),
                ));
                let mut hash_buf = [0u8; 13];
                let hash_str = silex_hash::css::encode_base36(hash_val, &mut hash_buf);
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

pub fn make_dynamic_val_for<P, S>(source: S) -> Rx<String>
where
    S: IntoRx,
    S::Value: Clone + Sized + types::ValidFor<P> + Display + 'static,
    S::RxType: silex_core::traits::RxGet<Value = S::Value> + 'static,
{
    let signal = source.into_rx();
    Rx::derive(Box::new(move || format!("{}", signal.get())))
}
