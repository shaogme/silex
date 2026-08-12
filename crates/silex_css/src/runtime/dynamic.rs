use crate::{
    layers,
    runtime::{
        backend::{ActiveSheet, SheetBackend},
        platform::report,
        registry::{DocOp, apply_doc_op},
        template::{
            CssPart, dynamic_class_with_static, render_selector_with_static,
            render_static_template, replace_placeholders,
        },
    },
    source::IntoCssReactive,
    types,
};
use silex_core::{ErrorReporter, RuntimeInputs, Rx, SilexError, SilexResult};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom, AttrOp, IntoStorable, PendingAttribute},
    view::{ApplyAttributes, OwnerState, View, ViewErrorHandler, ViewOwner, ViewOwnerToken},
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

/// 一个需要在 owner 验证通过后注入的静态 CSS 模板。
#[doc(hidden)]
#[derive(Clone)]
pub struct StaticStyleTemplate {
    pub style_id: &'static str,
    pub template: &'static str,
    pub values: Vec<String>,
}

impl StaticStyleTemplate {
    pub fn new(style_id: &'static str, template: &'static str, values: Vec<String>) -> Self {
        Self {
            style_id,
            template,
            values,
        }
    }
}

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
    /// 当前持有这张动态样式表的 manager 数量。
    ///
    /// 同一 logical id 且内容相同的 owner 可以共享状态；因此不能用
    /// `Rc::strong_count` 判断最后一个 owner，也不能在任一 manager dispose
    /// 时直接摘表。
    leases: Cell<usize>,
}

impl DynamicStyleState {
    fn attach(&self) -> bool {
        if self.attached.get() {
            return true;
        }
        if !self.sheet.attach() {
            report("挂载动态样式表失败");
            return false;
        }
        // 先记状态再排队：`attached` 记的是**意图**，排进队列的增删一定会发生，
        // 于是「挂上了没有」不再取决于这一刻借不借得到注册表。此前借用冲突时
        // 直接 return，这张表就一直不在文档里，只有等它退休又被复用才会重试。
        self.attached.set(true);
        if let Some(adopted) = self.sheet.adopted() {
            apply_doc_op(DocOp::Add(adopted));
        }
        true
    }

    fn detach(&self) {
        if self.attached.replace(false)
            && let Some(adopted) = self.sheet.adopted()
        {
            // 借不到就排队，微任务里补做——不再静默跳过
            apply_doc_op(DocOp::Remove(adopted));
        }
        // 即使状态尚未成功标记为 attached，也要清理 create() 已经插入的
        // fallback 节点，避免挂载失败时留下孤立的 <style>。
        self.sheet.detach();
    }

    fn acquire(&self) -> bool {
        let previous = self.leases.get();
        self.leases.set(previous.saturating_add(1));
        if self.attach() {
            true
        } else {
            self.leases.set(previous);
            false
        }
    }

    fn release(&self) -> bool {
        let leases = self.leases.get();
        if leases == 0 {
            return false;
        }
        let remaining = leases - 1;
        self.leases.set(remaining);
        if remaining == 0 {
            self.detach();
            true
        } else {
            false
        }
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
        if let Some(state) = state_borrow.take()
            && state.release()
        {
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
                        // 复用一张退休的表：内容还在，但已经被摘出文档，得在
                        // 新 manager 获取 lease 时挂回去。
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
                leases: Cell::new(0),
            });
            reg.insert(state_id, Rc::downgrade(&state));
            Some(state)
        })
        .flatten() else {
            report(&format!("无法建立动态样式表 `{id}`，该规则不会生效"));
            return false;
        };

        self.take_and_retire();
        if !new_state.acquire() {
            return false;
        }
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
    static_styles: Vec<(&'static str, &'static str)>,
    static_values: Vec<String>,
    layer: &'static str,
}

impl<'scope> DynamicCss<'scope> {
    pub fn new(class_name: &'static str) -> Self {
        Self {
            class_name,
            vars: Vec::new(),
            rules: Vec::new(),
            static_styles: Vec::new(),
            static_values: Vec::new(),
            layer: layers::UTILITIES,
        }
    }

    /// Set the named layer used by this dynamic rule payload.
    pub fn with_layer(mut self, layer: &'static str) -> Self {
        self.layer = layer;
        self
    }

    /// Attach document-level style descriptors to this dynamic payload.
    ///
    /// The descriptors are injected only after the payload's complete source set
    /// has passed owner validation.  This keeps construction free of document
    /// side effects when a source belongs to a foreign runtime.
    pub fn with_static_style(mut self, style_id: &'static str, css: &'static str) -> Self {
        if !style_id.is_empty() && !css.is_empty() {
            self.static_styles.push((style_id, css));
        }
        self
    }

    /// Set the values used by all static placeholders in this payload.
    pub fn with_static_values(mut self, values: Vec<String>) -> Self {
        self.static_values = values;
        self
    }

    pub fn with_var<P, S>(
        mut self,
        var_name: &'static str,
        source: S,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Self>
    where
        P: types::CssProperty,
        S: IntoCssReactive<'scope>,
        S::Value: Clone + Sized + types::ValidFor<P> + Display + 'scope,
    {
        self.vars
            .push((var_name, make_property_val::<P, S>(source, error_handler)?));
        Ok(self)
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

    fn apply_to_element(
        &self,
        el: &Element,
        owner: &dyn ViewOwner<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let all_inputs = self.runtime_inputs();
        owner.validate_inputs(&all_inputs)?;
        let token = owner.token();

        for (style_id, css) in &self.static_styles {
            let rendered = render_static_template(css, &self.static_values);
            crate::inject_style(style_id, &rendered);
        }

        el.class_list().add_1(self.class_name)?;

        if !self.vars.is_empty() {
            let vars = self.vars.clone();
            let vars_for_effect = vars.clone();
            let mut inputs = RuntimeInputs::new();
            for (_, getter) in &vars {
                inputs.extend(&getter.runtime_inputs());
            }
            let el_clone = el.clone();
            token.effect_with_previous_from(
                inputs,
                Box::new(
                    move |previous: Option<&Vec<Option<String>>>| -> SilexResult<
                        Vec<Option<String>>,
                    > {
                        let values: Vec<String> = vars_for_effect
                            .iter()
                            .map(|(_, getter)| getter.get())
                            .collect::<SilexResult<_>>()?;
                        if let Some(style) = element_style(&el_clone) {
                            for (index, ((name, _), value)) in
                                vars_for_effect.iter().zip(values.iter()).enumerate()
                            {
                                let old_value = previous
                                    .and_then(|values| values.get(index))
                                    .and_then(Option::as_deref);
                                if old_value != Some(value.as_str()) {
                                    style.set_property(name, value)?;
                                }
                            }
                        }
                        Ok(values.into_iter().map(Some).collect())
                    },
                ),
                error_handler,
            )?;

            let names: Vec<&'static str> = vars.iter().map(|(name, _)| *name).collect();
            let el_clone = el.clone();
            owner.on_cleanup(
                Box::new(move || -> SilexResult<()> {
                    let mut first_error = None;
                    if let Some(style) = element_style(&el_clone) {
                        for name in names {
                            if let Err(error) = style.remove_property(name) {
                                first_error.get_or_insert_with(|| SilexError::from(error));
                            }
                        }
                    }
                    first_error.map_or(Ok(()), Err)
                }),
                error_handler,
            )?;
        }

        let static_values = self.static_values.clone();
        for (parts, getters) in self.rules.clone() {
            let mut inputs = RuntimeInputs::new();
            for getter in &getters {
                inputs.extend(&getter.runtime_inputs());
            }
            let manager = Rc::new(DynamicStyleManager::new());
            let manager_for_effect = manager.clone();
            let current_class = token.owner_state(None::<String>)?;
            let current_class_for_effect = current_class.clone();
            let el_clone = el.clone();
            let base_class = self.class_name;
            let layer = self.layer;
            let static_values_for_effect = static_values.clone();
            token.effect_with_previous_from(
                inputs,
                Box::new(move |previous: Option<&String>| -> SilexResult<String> {
                    let current_vals: Vec<String> = getters
                        .iter()
                        .map(|getter| getter.get())
                        .collect::<SilexResult<_>>()?;
                    let dyn_class = dynamic_class_with_static(
                        base_class,
                        parts,
                        &current_vals,
                        &static_values_for_effect,
                    );
                    if previous.is_some_and(|class| class == &dyn_class) {
                        return Ok(dyn_class);
                    }

                    let rule = render_layered_selector(
                        layer,
                        parts,
                        &dyn_class,
                        &current_vals,
                        &static_values_for_effect,
                    );
                    if !manager_for_effect.update(&dyn_class, &rule) {
                        return Err(SilexError::Dom("无法更新动态样式表".into()));
                    }
                    el_clone.class_list().add_1(&dyn_class)?;
                    if let Some(old_class) = previous {
                        el_clone.class_list().remove_1(old_class)?;
                    }
                    current_class_for_effect.update(|class| *class = Some(dyn_class.clone()))?;
                    Ok(dyn_class)
                }),
                error_handler,
            )?;

            let manager_for_cleanup = manager.clone();
            let current_class_for_cleanup = current_class.clone();
            let el_clone = el.clone();
            owner.on_cleanup(
                Box::new(move || -> SilexResult<()> {
                    let mut first_error = None;
                    if let Some(class_name) = current_class_for_cleanup.take_for_cleanup().flatten()
                        && let Err(error) = el_clone.class_list().remove_1(&class_name)
                    {
                        first_error = Some(SilexError::from(error));
                    }
                    manager_for_cleanup.dispose();
                    first_error.map_or(Ok(()), Err)
                }),
                error_handler,
            )?;
        }

        let class_name = self.class_name;
        let el_clone = el.clone();
        owner.on_cleanup(
            Box::new(move || -> SilexResult<()> {
                el_clone
                    .class_list()
                    .remove_1(class_name)
                    .map_err(SilexError::from)
            }),
            error_handler,
        )?;
        Ok(())
    }
}

impl<'scope> ApplyToDom<'scope> for DynamicCss<'scope> {
    fn apply(
        &self,
        el: &Element,
        _target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.apply_to_element(el, owner, error_handler)
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        let inputs = self.runtime_inputs();
        AttrOp::custom_with_inputs(inputs, move |el, owner, error_handler| {
            self.apply_to_element(el, owner, error_handler)
        })
    }
}

impl<'scope> IntoStorable<'scope> for DynamicCss<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct StyledDynamicRule<'scope> {
    variant_group: Option<usize>,
    variant_key: Option<&'static str>,
    class_name: &'static str,
    parts: &'static [CssPart],
    getters: Vec<CssVariableGetter<'scope>>,
    static_values: Vec<String>,
}

impl<'scope> StyledDynamicRule<'scope> {
    pub fn new(
        variant_group: Option<usize>,
        variant_key: Option<&'static str>,
        class_name: &'static str,
        parts: &'static [CssPart],
        getters: Vec<CssVariableGetter<'scope>>,
    ) -> Self {
        Self {
            variant_group,
            variant_key,
            class_name,
            parts,
            getters,
            static_values: Vec::new(),
        }
    }

    pub fn with_static_values(mut self, values: Vec<String>) -> Self {
        self.static_values = values;
        self
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct StyledVariantGroup<'scope> {
    source: CssVariableGetter<'scope>,
    classes: Vec<(&'static str, &'static str)>,
}

impl<'scope> StyledVariantGroup<'scope> {
    pub fn new(
        source: CssVariableGetter<'scope>,
        classes: Vec<(&'static str, &'static str)>,
    ) -> Self {
        Self { source, classes }
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct StyledVariantBinding<'scope> {
    layer: &'static str,
    rules: Vec<StyledDynamicRule<'scope>>,
    groups: Vec<StyledVariantGroup<'scope>>,
    static_styles: Vec<(&'static str, &'static str)>,
    static_templates: Vec<StaticStyleTemplate>,
    property_getters: Vec<CssVariableGetter<'scope>>,
}

struct StyledRuleState {
    manager: Option<Rc<DynamicStyleManager>>,
    current_class: Option<String>,
}

impl<'scope> StyledVariantBinding<'scope> {
    pub fn new(
        layer: &'static str,
        rules: Vec<StyledDynamicRule<'scope>>,
        groups: Vec<StyledVariantGroup<'scope>>,
    ) -> Self {
        Self {
            layer,
            rules,
            groups,
            static_styles: Vec::new(),
            static_templates: Vec::new(),
            property_getters: Vec::new(),
        }
    }

    /// Attach styled static descriptors and declaration getters to one
    /// owner-bound validation boundary.
    pub fn with_static_styles(
        mut self,
        static_styles: Vec<(&'static str, &'static str)>,
        property_getters: Vec<CssVariableGetter<'scope>>,
    ) -> Self {
        self.static_styles = static_styles;
        self.property_getters = property_getters;
        self
    }

    /// Attach static templates whose values are evaluated at component construction.
    pub fn with_static_templates(mut self, templates: Vec<StaticStyleTemplate>) -> Self {
        self.static_templates = templates;
        self
    }

    pub fn into_op(self) -> AttrOp<'scope> {
        let inputs = self.runtime_inputs();
        AttrOp::custom_with_inputs(inputs, move |element, owner, error_handler| {
            self.mount_to_element(element, owner, error_handler)
        })
    }

    fn runtime_inputs(&self) -> RuntimeInputs {
        let mut inputs = RuntimeInputs::new();
        for group in &self.groups {
            inputs.extend(&group.source.runtime_inputs());
        }
        for rule in &self.rules {
            for getter in &rule.getters {
                inputs.extend(&getter.runtime_inputs());
            }
        }
        for getter in &self.property_getters {
            inputs.extend(&getter.runtime_inputs());
        }
        inputs
    }

    fn mount_to_element(
        &self,
        element: &Element,
        owner: &ViewOwnerToken<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let inputs = self.runtime_inputs();
        owner.validate_inputs(&inputs)?;

        for (style_id, css) in &self.static_styles {
            if !style_id.is_empty() && !css.is_empty() {
                crate::inject_style(style_id, css);
            }
        }
        for template in &self.static_templates {
            if !template.style_id.is_empty() && !template.template.is_empty() {
                let css = render_static_template(template.template, &template.values);
                crate::inject_style(template.style_id, &css);
            }
        }

        if self.rules.is_empty() && self.groups.is_empty() {
            return Ok(());
        }

        let rules = self.rules.clone();
        let groups = self.groups.clone();
        let layer = self.layer;
        let states = owner.owner_state(
            (0..rules.len())
                .map(|_| StyledRuleState {
                    manager: None,
                    current_class: None,
                })
                .collect::<Vec<_>>(),
        )?;
        let variant_classes = owner.owner_state(vec![None; groups.len()])?;
        let states_for_effect = states.clone();
        let variant_classes_for_effect = variant_classes.clone();
        let element_for_effect = element.clone();
        owner.effect_from(
            inputs,
            Box::new(move || -> SilexResult<()> {
                let active_variants: Vec<String> = groups
                    .iter()
                    .map(|group| group.source.get().map(|value| value.to_lowercase()))
                    .collect::<Result<_, _>>()?;
                update_styled_variant_classes(
                    &element_for_effect,
                    &groups,
                    &active_variants,
                    &variant_classes_for_effect,
                )?;
                update_styled_dynamic_rules(
                    &element_for_effect,
                    &rules,
                    layer,
                    &active_variants,
                    &states_for_effect,
                )?;
                Ok(())
            }),
            error_handler,
        )?;

        let element_for_cleanup = element.clone();
        owner.on_cleanup(
            Box::new(move || -> SilexResult<()> {
                let mut first_error = None;
                let mut classes = variant_classes.take_for_cleanup().unwrap_or_default();
                for class in classes.iter_mut().filter_map(Option::take) {
                    if let Err(error) = element_for_cleanup.class_list().remove_1(class) {
                        first_error.get_or_insert_with(|| SilexError::from(error));
                    }
                }

                let mut states = states.take_for_cleanup().unwrap_or_default();
                for state in states.iter_mut() {
                    if let Some(class) = state.current_class.take()
                        && let Err(error) = element_for_cleanup.class_list().remove_1(&class)
                    {
                        first_error.get_or_insert_with(|| SilexError::from(error));
                    }
                    if let Some(manager) = &state.manager {
                        manager.dispose();
                    }
                }
                first_error.map_or(Ok(()), Err)
            }),
            error_handler,
        )?;
        Ok(())
    }
}

fn update_styled_variant_classes(
    element: &Element,
    groups: &[StyledVariantGroup<'_>],
    active_variants: &[String],
    current_classes: &OwnerState<'_, Vec<Option<&'static str>>>,
) -> SilexResult<()> {
    current_classes.update(|current_classes| {
        for (index, group) in groups.iter().enumerate() {
            let next_class = group
                .classes
                .iter()
                .find(|(key, _)| {
                    active_variants
                        .get(index)
                        .is_some_and(|active| active == key)
                })
                .map(|(_, class)| *class);
            if current_classes[index] == next_class {
                continue;
            }
            if let Some(next_class) = next_class {
                element.class_list().add_1(next_class)?;
            }
            let old_class = current_classes[index];
            current_classes[index] = next_class;
            if let Some(old_class) = old_class {
                element.class_list().remove_1(old_class)?;
            }
        }
        Ok(())
    })?
}

fn update_styled_dynamic_rules(
    element: &Element,
    rules: &[StyledDynamicRule<'_>],
    layer: &'static str,
    active_variants: &[String],
    states: &OwnerState<'_, Vec<StyledRuleState>>,
) -> SilexResult<()> {
    for (index, rule) in rules.iter().enumerate() {
        let active = match (rule.variant_group, rule.variant_key) {
            (None, None) => true,
            (Some(group), Some(key)) => active_variants
                .get(group)
                .is_some_and(|active| active == key),
            _ => false,
        };

        if !active {
            let old_class = states.update(|states| {
                let state = &mut states[index];
                if let Some(manager) = &state.manager {
                    manager.dispose();
                }
                state.current_class.take()
            })?;
            if let Some(old_class) = old_class {
                element.class_list().remove_1(&old_class)?;
            }
            continue;
        }

        let manager = states.update(|states| {
            let state = &mut states[index];
            state
                .manager
                .get_or_insert_with(|| Rc::new(DynamicStyleManager::new()))
                .clone()
        })?;
        let Some(next_class) = dynamic_rule_class_with_static(
            &manager,
            layer,
            rule.class_name,
            rule.parts,
            &rule.getters,
            &rule.static_values,
        )?
        else {
            return Err(SilexError::Dom("无法更新动态样式表".into()));
        };

        let old_class = states.with(|states| states[index].current_class.clone())?;
        if old_class.as_deref() == Some(next_class.as_str()) {
            continue;
        }
        element.class_list().add_1(&next_class)?;
        states.update(|states| states[index].current_class = Some(next_class.clone()))?;
        if let Some(old_class) = old_class {
            element.class_list().remove_1(&old_class)?;
        }
    }
    Ok(())
}

/// A document-level dynamic stylesheet binding with no DOM representation.
///
/// The macro layer only constructs this descriptor.  Stylesheet managers and
/// effects are created by `GlobalStyleView` during mount, after the owner has
/// validated every source input.
#[doc(hidden)]
#[derive(Clone)]
pub struct GlobalStyleBinding<'scope> {
    pub style_id: &'static str,
    pub parts: &'static [CssPart],
    /// `None` means `parts` already contains its layer wrapper.
    pub layer: Option<&'static str>,
    pub positional: Vec<CssVariableGetter<'scope>>,
    pub replacements: Vec<(String, CssVariableGetter<'scope>)>,
    pub static_values: Vec<String>,
    pub static_replacements: Vec<(String, String)>,
}

/// 绑定 owner 的动态样式注入参数。
#[doc(hidden)]
pub struct ManagedDynamicStyle<'scope> {
    pub style_id: String,
    pub layer: Option<&'static str>,
    pub parts: &'static [CssPart],
    pub positional: Vec<CssVariableGetter<'scope>>,
    pub replacements: Vec<(String, CssVariableGetter<'scope>)>,
    pub static_values: Vec<String>,
    pub static_replacements: Vec<(String, String)>,
}

impl<'scope> GlobalStyleBinding<'scope> {
    pub fn new(
        style_id: &'static str,
        parts: &'static [CssPart],
        positional: Vec<CssVariableGetter<'scope>>,
        replacements: Vec<(String, CssVariableGetter<'scope>)>,
    ) -> Self {
        Self {
            style_id,
            parts,
            layer: None,
            positional,
            replacements,
            static_values: Vec::new(),
            static_replacements: Vec::new(),
        }
    }

    /// Mark this binding as a raw dynamic rule that needs a runtime layer wrapper.
    pub fn with_layer(mut self, layer: &'static str) -> Self {
        self.layer = Some(layer);
        self
    }

    pub fn with_static_values(mut self, values: Vec<String>) -> Self {
        self.static_values = values;
        self
    }

    pub fn with_static_replacements(mut self, replacements: Vec<(String, String)>) -> Self {
        self.static_replacements = replacements;
        self
    }

    fn runtime_inputs(&self) -> RuntimeInputs {
        let mut inputs = RuntimeInputs::new();
        for getter in &self.positional {
            inputs.extend(&getter.runtime_inputs());
        }
        for (_, getter) in &self.replacements {
            inputs.extend(&getter.runtime_inputs());
        }
        inputs
    }
}

/// An owner-bound document stylesheet that does not create a DOM node.
///
/// Static descriptors are injected only after the complete input set has been
/// validated.  Dynamic bindings delegate manager/effect/cleanup ownership to
/// `inject_managed_dynamic_style`.
#[doc(hidden)]
#[derive(Clone)]
pub struct GlobalStyleView<'scope> {
    static_styles: Vec<(&'static str, &'static str)>,
    bindings: Vec<GlobalStyleBinding<'scope>>,
}

impl<'scope> GlobalStyleView<'scope> {
    pub fn new(
        static_styles: Vec<(&'static str, &'static str)>,
        bindings: Vec<GlobalStyleBinding<'scope>>,
    ) -> Self {
        Self {
            static_styles,
            bindings,
        }
    }

    fn mount_inner(
        &self,
        owner: &dyn ViewOwner<'scope>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let mut inputs = RuntimeInputs::new();
        for binding in &self.bindings {
            inputs.extend(&binding.runtime_inputs());
        }
        owner.validate_inputs(&inputs)?;

        for (style_id, css) in &self.static_styles {
            if !style_id.is_empty() && !css.is_empty() {
                crate::inject_style(style_id, css);
            }
        }

        for binding in &self.bindings {
            let style_id = unique_dynamic_style_id(binding.style_id);
            inject_managed_dynamic_style(
                owner,
                error_handler,
                ManagedDynamicStyle {
                    style_id,
                    layer: binding.layer,
                    parts: binding.parts,
                    positional: binding.positional.clone(),
                    replacements: binding.replacements.clone(),
                    static_values: binding.static_values.clone(),
                    static_replacements: binding.static_replacements.clone(),
                },
            )?;
        }
        Ok(())
    }
}

impl<'scope> ApplyAttributes<'scope> for GlobalStyleView<'scope> {}

impl<'scope> View<'scope> for GlobalStyleView<'scope> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        _parent: &web_sys::Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()> {
        self.mount_inner(owner, error_handler)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        _parent: &web_sys::Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ViewErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount_inner(owner, error_handler)
    }
}

pub fn make_property_val<'scope, P, S>(
    source: S,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<Rx<'scope, String>>
where
    P: types::CssProperty,
    S: IntoCssReactive<'scope>,
    S::Value: Clone + Sized + types::ValidFor<P> + Display + 'scope,
{
    let source = source.into_css_reactive();
    source.map(|value| value.to_string(), error_handler)
}

/// 一条带动态选择器的组件规则：算出本轮类名、把规则写进独占样式表、返回类名。
///
/// `styled!` 此前把这段逻辑整个展开在宏产物里（而且为变体分支复制了一份），
/// 中间还夹着 `res.replace(class_name, &dyn_class)` 这种子串替换。
pub fn dynamic_rule_class(
    manager: &DynamicStyleManager,
    layer: &'static str,
    base_class: &str,
    parts: &'static [CssPart],
    getters: &[CssVariableGetter<'_>],
) -> SilexResult<Option<String>> {
    dynamic_rule_class_with_static(manager, layer, base_class, parts, getters, &[])
}

pub fn dynamic_rule_class_with_static(
    manager: &DynamicStyleManager,
    layer: &'static str,
    base_class: &str,
    parts: &'static [CssPart],
    getters: &[CssVariableGetter<'_>],
    static_values: &[String],
) -> SilexResult<Option<String>> {
    let vals: Vec<String> = getters
        .iter()
        .map(|getter| getter.get())
        .collect::<SilexResult<_>>()?;
    let dyn_class = dynamic_class_with_static(base_class, parts, &vals, static_values);
    let rule = render_layered_selector(layer, parts, &dyn_class, &vals, static_values);
    Ok(manager.update(&dyn_class, &rule).then_some(dyn_class))
}

fn render_layered_selector(
    layer: &'static str,
    parts: &[CssPart],
    class: &str,
    vals: &[String],
    static_values: &[String],
) -> String {
    let rule = render_selector_with_static(parts, class, vals, static_values);
    layers::wrap_dynamic(layer, &rule)
}

/// Helper function to inject managed dynamic style with reactive variable replacements.
///
/// 模板里有两类运行时片段：
///
/// - `parts` 里的 `CssPart::SelectorVal(i)`：**选择器**里的片段
///   （`.x $theme { … }`）。
///   全局样式没有可挂 CSS 变量的元素，只能把值拼进规则文本。
/// - `replacements`：具名的 `var(--slx-dyn-N)`，用于**声明值**里的片段。这段
///   模板要先过一遍 lightningcss，位置信息在那之后不复存在，只能按文本找；但
///   替换是一遍扫描完成的，写进去的值不会再被当成占位符。
pub fn inject_managed_dynamic_style<'scope>(
    owner: &dyn ViewOwner<'scope>,
    error_handler: ViewErrorHandler<'scope>,
    style: ManagedDynamicStyle<'scope>,
) -> SilexResult<()> {
    let ManagedDynamicStyle {
        style_id,
        layer,
        parts,
        positional,
        replacements,
        static_values,
        static_replacements,
    } = style;
    let mut inputs = RuntimeInputs::new();
    for getter in &positional {
        inputs.extend(&getter.runtime_inputs());
    }
    for (_, getter) in &replacements {
        inputs.extend(&getter.runtime_inputs());
    }
    owner.validate_inputs(&inputs)?;

    let manager = Rc::new(DynamicStyleManager::new());
    let style_id_str = style_id;
    let manager_for_cleanup = manager.clone();
    owner.on_cleanup(
        Box::new(move || {
            manager_for_cleanup.dispose();
            Ok(())
        }),
        error_handler,
    )?;

    let manager_for_effect = manager.clone();
    owner
        .effect_from(
            inputs,
            Box::new(move || -> SilexResult<()> {
                let vals: Vec<String> = positional
                    .iter()
                    .map(|getter| getter.get())
                    .collect::<SilexResult<_>>()?;
                // 全局样式没有组件类名，`CssPart::Class` 不会出现在这类模板里
                let res = render_selector_with_static(parts, "", &vals, &static_values);
                let mut pairs = static_replacements
                    .iter()
                    .map(|(pattern, value)| {
                        (
                            pattern.clone(),
                            crate::escape::declaration_value(value).into_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                pairs.extend(
                    replacements
                        .iter()
                        .map(|(pattern, getter)| {
                            getter.get().map(|value| {
                                (
                                    pattern.clone(),
                                    crate::escape::declaration_value(&value).into_owned(),
                                )
                            })
                        })
                        .collect::<SilexResult<Vec<_>>>()?,
                );
                let res = replace_placeholders(&res, &pairs);
                let res = match layer {
                    Some(layer) => layers::wrap_dynamic(layer, &res),
                    None => res,
                };
                if !manager_for_effect.update(&style_id_str, &res) {
                    return Err(SilexError::Dom("无法更新动态样式表".into()));
                }
                Ok(())
            }),
            error_handler,
        )
        .inspect_err(|_| manager.dispose())?;
    Ok(())
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
