use crate::{
    runtime::{
        backend::{ActiveSheet, SheetBackend},
        platform::report,
        registry::{DocOp, apply_doc_op},
        template::{CssPart, dynamic_class, render_selector, replace_placeholders},
    },
    source::IntoCssReactive,
    types,
};
use silex_core::{RuntimeInputs, Rx, error::handle_error};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom, AttrOp, IntoStorable},
    view::{ViewOwner, ViewOwnerToken},
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    fmt::Display,
    rc::{Rc, Weak},
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, SvgElement};

pub type CssVariableGetter<'scope> = Rx<'scope, String>;

/// 退休样式表的缓存上限。
///
/// 此前是 128，且退休状态仍以 `Rc` 留在队列里 → `Drop` 不触发 → `remove_sheet`
/// 不执行 → 这些表**始终留在 `document.adoptedStyleSheets` 上**，每一张都参与
/// 样式匹配。现在退休即摘出文档（只保留已解析好的表对象供复用），常驻成本降到
/// 零，上限本身也随之收小。
pub(crate) const CACHE_LIMIT: usize = 32;

thread_local! {
    static DYNAMIC_STYLE_REGISTRY: RefCell<HashMap<String, Weak<DynamicStyleState>>> = RefCell::new(HashMap::new());
    static RETIRED_STYLES: RefCell<VecDeque<Rc<DynamicStyleState>>> = const { RefCell::new(VecDeque::new()) };
    /// `DYNAMIC_STYLE_REGISTRY` 正被借用时来不及注销的 id
    static PENDING_UNREGISTER: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static NEXT_DYNAMIC_STYLE_ID: Cell<u64> = const { Cell::new(0) };
}

/// Manages an injected stylesheet uniquely for a component instance.
pub(crate) struct DynamicStyleState {
    pub id: String,
    logical_id: String,
    pub sheet: ActiveSheet,
    content: RefCell<String>,
    /// 当前是否挂在 `document.adoptedStyleSheets` 上
    attached: std::cell::Cell<bool>,
}

impl DynamicStyleState {
    fn attach(&self) {
        if self.attached.get() {
            return;
        }
        // 先记状态再排队：`attached` 记的是**意图**，排进队列的增删一定会发生，
        // 于是「挂上了没有」不再取决于这一刻借不借得到注册表。此前借用冲突时
        // 直接 return，这张表就一直不在文档里，只有等它退休又被复用才会重试。
        self.attached.set(true);
        if let Some(adopted) = self.sheet.adopted() {
            apply_doc_op(DocOp::Add(adopted));
        }
    }

    fn detach(&self) {
        if !self.attached.get() {
            return;
        }
        self.attached.set(false);
        if let Some(adopted) = self.sheet.adopted() {
            // 借不到就排队，微任务里补做——不再静默跳过
            apply_doc_op(DocOp::Remove(adopted));
        }
        self.sheet.detach();
    }
}

impl Drop for DynamicStyleState {
    fn drop(&mut self) {
        // 1. Remove from document stylesheets
        self.detach();
        // 2. Remove from registry map
        //
        // `try_with`：线程退出时 TLS 析构器会来 `Drop` 退休队列里的状态，那时
        // 注册表本身可能已经没了——在析构器里 panic 会直接 abort 进程。
        let removed = DYNAMIC_STYLE_REGISTRY
            .try_with(|reg| match reg.try_borrow_mut() {
                Ok(mut reg) => {
                    reg.remove(&self.id);
                    true
                }
                Err(_) => false,
            })
            .unwrap_or(true);
        if !removed {
            let _ = PENDING_UNREGISTER.try_with(|p| p.borrow_mut().push(self.id.clone()));
        }
    }
}

/// 清空退休队列与注册表。测试用，见 `registry::reset_for_test`。
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn reset_for_test() {
    // 先把退休队列放干净：`Drop` 会回头去动另外两张表，不能在借用里做
    let retired: Vec<Rc<DynamicStyleState>> =
        RETIRED_STYLES.with(|r| r.borrow_mut().drain(..).collect());
    drop(retired);
    DYNAMIC_STYLE_REGISTRY.with(|reg| reg.borrow_mut().clear());
    PENDING_UNREGISTER.with(|p| p.borrow_mut().clear());
}

/// 拿到动态样式注册表，顺带把欠下的注销补上。
fn with_dynamic_registry<R>(
    f: impl FnOnce(&mut HashMap<String, Weak<DynamicStyleState>>) -> R,
) -> Option<R> {
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
    state: RefCell<Option<Rc<DynamicStyleState>>>,
}

impl Default for DynamicStyleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicStyleManager {
    pub const fn new() -> Self {
        Self {
            state: RefCell::new(None),
        }
    }

    /// Moves the current style state to the retired cache if it's the last active reference.
    fn take_and_retire(&self) {
        let Ok(mut state_borrow) = self.state.try_borrow_mut() else {
            return;
        };
        if let Some(state) = state_borrow.take() {
            // manager dispose 必须立即停止样式匹配。effect/cleanup closure 可能仍
            // 暂时持有 state，不能把 detach 推迟到最后一个 Rc drop。
            state.detach();
            // If strong_count is 1, it means this manager was the only one holding the style.
            if Rc::strong_count(&state) == 1 {
                // 退休 = 保留内容、移出 adoptedStyleSheets。表对象还在（复用时
                // 不必重新解析 CSS），但不再参与样式匹配。
                RETIRED_STYLES.with(|retired| {
                    let Ok(mut r) = retired.try_borrow_mut() else {
                        return;
                    };
                    r.push_back(state);
                    if r.len() > CACHE_LIMIT {
                        // This will drop the oldest retired state, potentially triggering DynamicStyleState::drop
                        r.pop_front();
                    }
                });
            }
        }
    }

    pub fn update(&self, id: &str, content: &str) -> bool {
        if let Ok(state_borrow) = self.state.try_borrow()
            && let Some(state) = state_borrow.as_ref()
            && state.logical_id == id
        {
            let same_content = state.content.borrow().as_str() == content;
            if same_content || Rc::strong_count(state) == 1 {
                if !same_content {
                    if !state.sheet.replace(content) {
                        return false;
                    }
                    *state.content.borrow_mut() = content.to_string();
                }
                return true;
            }
        }

        let Some(new_state) = with_dynamic_registry(|reg| {
            let existing = reg.get(id).and_then(Weak::upgrade);
            if let Some(state) = existing {
                if !state.attached.get() {
                    RETIRED_STYLES.with(|retired| {
                        if let Ok(mut r) = retired.try_borrow_mut()
                            && let Some(pos) = r.iter().position(|s| s.id == state.id)
                        {
                            r.remove(pos);
                        }
                    });
                    if state.sheet.replace(content) {
                        *state.content.borrow_mut() = content.to_string();
                        // 复用一张退休的表：内容还在，但已经被摘出文档，得挂回去
                        state.attach();
                        return Some(state);
                    }
                }

                // 同一逻辑 id 的相同内容可以共享；不同内容必须拆分成独立表，
                // 否则一个 active owner 的更新会静默覆盖另一个 owner。
                if state.content.borrow().as_str() == content {
                    return Some(state);
                }
            } else {
                reg.remove(id);
            }

            let sheet = ActiveSheet::create()?;
            if !sheet.replace(content) {
                return None;
            }

            let state_id = if reg.contains_key(id) {
                unique_dynamic_style_id("slx-dynamic")
            } else {
                id.to_string()
            };
            let state = Rc::new(DynamicStyleState {
                id: state_id.clone(),
                logical_id: id.to_string(),
                sheet,
                content: RefCell::new(content.to_string()),
                attached: std::cell::Cell::new(false),
            });
            state.attach();
            reg.insert(state_id, Rc::downgrade(&state));
            Some(state)
        })
        .flatten() else {
            report(&format!("无法建立动态样式表 `{id}`，该规则不会生效"));
            return false;
        };

        self.take_and_retire();
        if let Ok(mut state_borrow) = self.state.try_borrow_mut() {
            *state_borrow = Some(new_state);
        }
        true
    }

    pub fn dispose(&self) {
        self.take_and_retire();
    }
}

impl Drop for DynamicStyleManager {
    fn drop(&mut self) {
        self.take_and_retire();
    }
}

/// A structure representing a dynamic CSS class with reactive variables and dynamic rules.
#[derive(Clone)]
pub struct DynamicCss<'scope> {
    pub class_name: &'static str,
    pub vars: Vec<(&'static str, CssVariableGetter<'scope>)>,
    pub rules: Vec<(&'static [CssPart], Vec<CssVariableGetter<'scope>>)>,
}

impl<'scope> DynamicCss<'scope> {
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
        S: IntoCssReactive<'scope>,
        S::Value: Clone + Sized + types::ValidFor<P> + Display + 'scope,
    {
        self.vars
            .push((var_name, make_property_val::<P, S>(source)));
        self
    }

    pub fn with_rule(
        mut self,
        parts: &'static [CssPart],
        exprs: Vec<CssVariableGetter<'scope>>,
    ) -> Self {
        // Rule getters represent selector fragments. Dynamic declaration values
        // belong in `with_var`, which is applied through an element CSS variable.
        self.rules.push((parts, exprs));
        self
    }

    pub(crate) fn runtime_inputs(&self) -> RuntimeInputs {
        let mut inputs = RuntimeInputs::new();
        for (_, getter) in &self.vars {
            inputs.extend(&getter.runtime_inputs());
        }
        for (_, getters) in &self.rules {
            for getter in getters {
                inputs.extend(&getter.runtime_inputs());
            }
        }
        inputs
    }

    fn apply_to_element(&self, el: &Element, owner: &dyn ViewOwner<'scope>) {
        let all_inputs = self.runtime_inputs();
        if let Err(error) = owner.validate_inputs(&all_inputs) {
            handle_error(error);
            return;
        }

        let _ = el.class_list().add_1(self.class_name);

        if !self.vars.is_empty() {
            let vars = self.vars.clone();
            let vars_for_effect = vars.clone();
            let mut inputs = RuntimeInputs::new();
            for (_, getter) in &vars {
                inputs.extend(&getter.runtime_inputs());
            }
            let previous = Rc::new(RefCell::new(vec![None::<String>; vars.len()]));
            let previous_for_effect = previous.clone();
            let el_clone = el.clone();
            owner.effect_from(
                inputs,
                Box::new(move || {
                    let values: Vec<String> = vars_for_effect
                        .iter()
                        .map(|(_, getter)| getter.get())
                        .collect();
                    let mut previous = previous_for_effect.borrow_mut();
                    if let Some(style) = element_style(&el_clone) {
                        for (index, ((name, _), value)) in
                            vars_for_effect.iter().zip(values.iter()).enumerate()
                        {
                            if previous[index].as_deref() != Some(value) {
                                let _ = style.set_property(name, value);
                            }
                        }
                    }
                    *previous = values.into_iter().map(Some).collect();
                }),
            );

            let names: Vec<&'static str> = vars.iter().map(|(name, _)| *name).collect();
            let el_clone = el.clone();
            owner.on_cleanup(Box::new(move || {
                if let Some(style) = element_style(&el_clone) {
                    for name in names {
                        let _ = style.remove_property(name);
                    }
                }
            }));
        }

        for (parts, getters) in self.rules.clone() {
            let mut inputs = RuntimeInputs::new();
            for getter in &getters {
                inputs.extend(&getter.runtime_inputs());
            }
            let manager = Rc::new(DynamicStyleManager::new());
            let manager_for_effect = manager.clone();
            let current_class = Rc::new(RefCell::new(None::<String>));
            let current_class_for_effect = current_class.clone();
            let el_clone = el.clone();
            let base_class = self.class_name;
            owner.effect_from(
                inputs,
                Box::new(move || {
                    let current_vals: Vec<String> =
                        getters.iter().map(|getter| getter.get()).collect();
                    let dyn_class = dynamic_class(base_class, parts, &current_vals);
                    let mut current_class = current_class_for_effect.borrow_mut();
                    if current_class.as_deref() == Some(dyn_class.as_str()) {
                        return;
                    }

                    let rule = render_selector(parts, &dyn_class, &current_vals);
                    if !manager_for_effect.update(&dyn_class, &rule) {
                        return;
                    }
                    let _ = el_clone.class_list().add_1(&dyn_class);
                    if let Some(old_class) = current_class.replace(dyn_class) {
                        let _ = el_clone.class_list().remove_1(&old_class);
                    }
                }),
            );

            let manager_for_cleanup = manager.clone();
            let current_class_for_cleanup = current_class.clone();
            let el_clone = el.clone();
            owner.on_cleanup(Box::new(move || {
                if let Some(class_name) = current_class_for_cleanup.borrow_mut().take() {
                    let _ = el_clone.class_list().remove_1(&class_name);
                }
                manager_for_cleanup.dispose();
            }));
        }

        let class_name = self.class_name;
        let el_clone = el.clone();
        owner.on_cleanup(Box::new(move || {
            let _ = el_clone.class_list().remove_1(class_name);
        }));
    }
}

impl<'scope> ApplyToDom<'scope> for DynamicCss<'scope> {
    fn apply(&self, el: &Element, _target: ApplyTarget, owner: &ViewOwnerToken<'scope>) {
        self.apply_to_element(el, owner);
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        let inputs = self.runtime_inputs();
        AttrOp::custom_with_inputs(inputs, move |el, owner| {
            self.apply_to_element(el, owner);
        })
    }
}

impl<'scope> IntoStorable<'scope> for DynamicCss<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

pub fn make_property_val<'scope, P, S>(source: S) -> Rx<'scope, String>
where
    P: types::CssProperty,
    S: IntoCssReactive<'scope>,
    S::Value: Clone + Sized + types::ValidFor<P> + Display + 'scope,
{
    source.into_css_reactive().map(|value| value.to_string())
}

/// 一条带动态选择器的组件规则：算出本轮类名、把规则写进独占样式表、返回类名。
///
/// `styled!` 此前把这段逻辑整个展开在宏产物里（而且为变体分支复制了一份），
/// 中间还夹着 `res.replace(class_name, &dyn_class)` 这种子串替换。
pub fn dynamic_rule_class(
    manager: &DynamicStyleManager,
    base_class: &str,
    parts: &'static [CssPart],
    getters: &[CssVariableGetter<'_>],
) -> String {
    let vals: Vec<String> = getters.iter().map(|g| g.get()).collect();
    let dyn_class = dynamic_class(base_class, parts, &vals);
    let rule = render_selector(parts, &dyn_class, &vals);
    manager.update(&dyn_class, &rule);
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
pub fn inject_managed_dynamic_style<'scope>(
    owner: &dyn ViewOwner<'scope>,
    style_id: impl Into<String>,
    parts: &'static [CssPart],
    positional: Vec<CssVariableGetter<'scope>>,
    replacements: Vec<(String, CssVariableGetter<'scope>)>,
) {
    let mut inputs = RuntimeInputs::new();
    for getter in &positional {
        inputs.extend(&getter.runtime_inputs());
    }
    for (_, getter) in &replacements {
        inputs.extend(&getter.runtime_inputs());
    }
    if let Err(error) = owner.validate_inputs(&inputs) {
        handle_error(error);
        return;
    }

    let manager = Rc::new(DynamicStyleManager::new());
    let manager_for_effect = manager.clone();
    let style_id_str = style_id.into();
    owner.effect_from(
        inputs,
        Box::new(move || {
            let vals: Vec<String> = positional.iter().map(|getter| getter.get()).collect();
            // 全局样式没有组件类名，`CssPart::Class` 不会出现在这类模板里
            let res = render_selector(parts, "", &vals);
            let pairs: Vec<(String, String)> = replacements
                .iter()
                .map(|(pattern, getter)| {
                    (
                        pattern.clone(),
                        crate::escape::declaration_value(&getter.get()).into_owned(),
                    )
                })
                .collect();
            let res = replace_placeholders(&res, &pairs);
            manager_for_effect.update(&style_id_str, &res);
        }),
    );
    let manager_for_cleanup = manager.clone();
    owner.on_cleanup(Box::new(move || manager_for_cleanup.dispose()));
}

fn element_style(el: &Element) -> Option<web_sys::CssStyleDeclaration> {
    el.dyn_ref::<HtmlElement>()
        .map(|element| element.style())
        .or_else(|| el.dyn_ref::<SvgElement>().map(|element| element.style()))
}

pub(crate) fn unique_dynamic_style_id(prefix: &str) -> String {
    let id = NEXT_DYNAMIC_STYLE_ID.with(|next| {
        let id = next.get();
        next.set(id.wrapping_add(1));
        id
    });
    format!("{prefix}-{id}")
}
