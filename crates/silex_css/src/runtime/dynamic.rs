use crate::{
    runtime::{
        registry::{queue_removal, with_document_registry},
        sheet::{Sheet, report},
        template::{CssPart, dynamic_class, render, replace_placeholders},
    },
    types,
};
use silex_core::{prelude::*, traits::RxGet};
use silex_dom::prelude::*;
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    fmt::Display,
    rc::{Rc, Weak},
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, SvgElement};

pub type CssVariableGetter = Rx<String>;

/// 退休样式表的缓存上限。
///
/// 此前是 128，且退休状态仍以 `Rc` 留在队列里 → `Drop` 不触发 → `remove_sheet`
/// 不执行 → 这些表**始终留在 `document.adoptedStyleSheets` 上**，每一张都参与
/// 样式匹配。现在退休即摘出文档（只保留已解析好的表对象供复用），常驻成本降到
/// 零，上限本身也随之收小。
const CACHE_LIMIT: usize = 32;

thread_local! {
    static DYNAMIC_STYLE_REGISTRY: RefCell<HashMap<String, Weak<DynamicStyleState>>> = RefCell::new(HashMap::new());
    static RETIRED_STYLES: RefCell<VecDeque<Rc<DynamicStyleState>>> = const { RefCell::new(VecDeque::new()) };
    /// `DYNAMIC_STYLE_REGISTRY` 正被借用时来不及注销的 id
    static PENDING_UNREGISTER: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Manages an injected stylesheet uniquely for a component instance.
pub(crate) struct DynamicStyleState {
    pub id: String,
    pub sheet: Sheet,
    /// 当前是否挂在 `document.adoptedStyleSheets` 上
    attached: std::cell::Cell<bool>,
}

impl DynamicStyleState {
    fn attach(&self) {
        if self.attached.get() {
            return;
        }
        if let Some(adopted) = self.sheet.adopted() {
            let adopted = adopted.clone();
            if with_document_registry(|dr| dr.add_sheet(adopted)).is_none() {
                report("挂载动态样式表时借用冲突");
                return;
            }
        }
        self.attached.set(true);
    }

    fn detach(&self) {
        if !self.attached.get() {
            return;
        }
        self.attached.set(false);
        let Some(adopted) = self.sheet.adopted() else {
            return;
        };
        if with_document_registry(|dr| dr.remove_sheet(adopted)).is_none() {
            // 借不到就排队，微任务里补做——不再静默跳过
            queue_removal(adopted.clone());
        }
    }
}

impl Drop for DynamicStyleState {
    fn drop(&mut self) {
        // 1. Remove from document stylesheets
        self.detach();
        self.sheet.detach();
        // 2. Remove from registry map
        let removed = DYNAMIC_STYLE_REGISTRY.with(|reg| match reg.try_borrow_mut() {
            Ok(mut reg) => {
                reg.remove(&self.id);
                true
            }
            Err(_) => false,
        });
        if !removed {
            PENDING_UNREGISTER.with(|p| p.borrow_mut().push(self.id.clone()));
        }
    }
}

/// 拿到动态样式注册表，顺带把欠下的注销补上。
fn with_dynamic_registry<R>(f: impl FnOnce(&mut HashMap<String, Weak<DynamicStyleState>>) -> R) -> Option<R> {
    DYNAMIC_STYLE_REGISTRY.with(|reg| {
        let Ok(mut reg) = reg.try_borrow_mut() else {
            return None;
        };
        let owed = PENDING_UNREGISTER.with(|p| p.borrow_mut().drain(..).collect::<Vec<_>>());
        for id in &owed {
            reg.remove(id);
        }
        Some(f(&mut reg))
    })
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

    /// Moves the current style state to the retired cache if it's the last active reference.
    fn take_and_retire(&mut self) {
        if let Some(state) = self.state.take() {
            // If strong_count is 1, it means this manager was the only one holding the style.
            if Rc::strong_count(&state) == 1 {
                // 退休 = 保留内容、移出 adoptedStyleSheets。表对象还在（复用时
                // 不必重新解析 CSS），但不再参与样式匹配。
                state.detach();
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
            state.sheet.replace(content);
            return;
        }

        let Some(new_state) = with_dynamic_registry(|reg| {
            if let Some(weak) = reg.get(id)
                && let Some(state) = weak.upgrade()
            {
                RETIRED_STYLES.with(|retired| {
                    let mut r = retired.borrow_mut();
                    if let Some(pos) = r.iter().position(|s| s.id == id) {
                        r.remove(pos);
                    }
                });
                state.sheet.replace(content);
                // 复用一张退休的表：内容还在，但已经被摘出文档，得挂回去
                state.attach();
                return Some(state);
            }
            let sheet = Sheet::new()?;
            sheet.replace(content);

            let state = Rc::new(DynamicStyleState {
                id: id.to_string(),
                sheet,
                attached: std::cell::Cell::new(false),
            });
            state.attach();
            reg.insert(id.to_string(), Rc::downgrade(&state));
            Some(state)
        })
        .flatten() else {
            report(&format!("无法建立动态样式表 `{id}`，该规则不会生效"));
            return;
        };

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
    pub rules: Vec<(&'static [CssPart], Vec<CssVariableGetter>)>,
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

    pub fn with_rule(
        mut self,
        parts: &'static [CssPart],
        exprs: Vec<CssVariableGetter>,
    ) -> Self {
        self.rules.push((parts, exprs));
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
        for (parts, getters) in self.rules.clone() {
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

                let dyn_class = dynamic_class(base_class, parts, &current_vals);

                let prev_class = prev.as_ref().map(|(_, c)| c);
                if Some(&dyn_class) != prev_class {
                    if let Some(old_class) = prev_class {
                        let _ = el_clone.class_list().remove_1(old_class);
                    }
                    let _ = el_clone.class_list().add_1(&dyn_class);

                    let rule = render(parts, &dyn_class, &current_vals);
                    if let Ok(mut opt) = manager.try_borrow_mut()
                        && let Some(mgr) = opt.as_mut()
                    {
                        mgr.update(&dyn_class, &rule);
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

/// 一条带动态选择器的组件规则：算出本轮类名、把规则写进独占样式表、返回类名。
///
/// `styled!` 此前把这段逻辑整个展开在宏产物里（而且为变体分支复制了一份），
/// 中间还夹着 `res.replace(class_name, &dyn_class)` 这种子串替换。
pub fn dynamic_rule_class(
    manager: &Rc<RefCell<Option<DynamicStyleManager>>>,
    base_class: &str,
    parts: &'static [CssPart],
    getters: &[CssVariableGetter],
) -> String {
    let vals: Vec<String> = getters.iter().map(|g| g.get()).collect();
    let dyn_class = dynamic_class(base_class, parts, &vals);
    let rule = render(parts, &dyn_class, &vals);
    if let Ok(mut opt) = manager.try_borrow_mut()
        && let Some(m) = opt.as_mut()
    {
        m.update(&dyn_class, &rule);
    }
    dyn_class
}

/// Helper function to inject managed dynamic style with reactive variable replacements.
///
/// 模板里有两类运行时片段：
///
/// - `parts` 里的 `CssPart::Val(i)`：**选择器**里的片段（`.x $theme { … }`）。
///   全局样式没有可挂 CSS 变量的元素，只能把值拼进规则文本。
/// - `replacements`：具名的 `var(--slx-dyn-N)`，用于**声明值**里的片段。这段
///   模板要先过一遍 lightningcss，位置信息在那之后不复存在，只能按文本找；但
///   替换是一遍扫描完成的，写进去的值不会再被当成占位符。
pub fn inject_managed_dynamic_style(
    style_id: impl Into<String>,
    parts: &'static [CssPart],
    positional: Vec<CssVariableGetter>,
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
        let vals: Vec<String> = positional.iter().map(|g| g.get()).collect();
        // 全局样式没有组件类名，`CssPart::Class` 不会出现在这类模板里
        let res = render(parts, "", &vals);
        let pairs: Vec<(String, String)> = replacements
            .iter()
            .map(|(pattern, getter)| (pattern.clone(), getter.get()))
            .collect();
        let res = replace_placeholders(&res, &pairs);
        if let Ok(mut opt) = manager.try_borrow_mut()
            && let Some(m) = opt.as_mut()
        {
            m.update(&style_id_str, &res);
        }
    });
}
