use crate::{
    for_all_properties, inject_style,
    types::{
        ValidFor, props,
        props::{
            MarginBottom, MarginLeft, MarginRight, MarginTop, PaddingBottom, PaddingLeft,
            PaddingRight, PaddingTop,
        },
    },
};
use silex_core::{
    Rx, RxValueKind,
    reactivity::{Effect, on_cleanup},
    traits::{IntoRx, RxGet, RxValue},
};
use silex_dom::attribute::{ApplyTarget, ApplyToDom, IntoStorable, ReactiveApply};
use silex_hash::{
    css::{CssHasher, Normalized, encode_base36},
    css_hasher,
};
use std::{
    borrow::Cow,
    fmt::{Display, Write},
    hash::{Hash, Hasher},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, SvgElement};

pub(crate) type DynamicValue = Rc<dyn Fn() -> String>;
pub(crate) type StaticRule = (&'static str, Cow<'static, str>);
pub(crate) type DynamicRule = (&'static str, DynamicValue);

#[derive(Clone)]
enum StyleValue {
    Static(Cow<'static, str>),
    Dynamic(DynamicValue),
}

impl StyleValue {
    fn from_rx<V>(value: V) -> Self
    where
        V: IntoRx + RxValue + 'static,
        V::Value: Display + Clone + Sized + 'static,
        V::RxType: RxGet<Value = V::Value> + 'static,
    {
        if value.is_constant() {
            let signal = value.into_rx();
            Self::Static(Cow::Owned(format!("{}", signal.get())))
        } else {
            let signal = value.into_rx();
            Self::Dynamic(Rc::new(move || format!("{}", signal.get())))
        }
    }
}

#[derive(Clone)]
pub(crate) enum NestedRule {
    Media(&'static str, Style),
    Selector(&'static str, Style),
}

#[derive(Clone)]
pub struct Style {
    pub(crate) static_rules: Vec<StaticRule>,
    pub(crate) dynamic_rules: Vec<DynamicRule>,
    pub(crate) nested_rules: Vec<NestedRule>,
}

impl Default for Style {
    fn default() -> Self {
        Self::new()
    }
}

impl Style {
    pub fn new() -> Self {
        Self {
            static_rules: Vec::new(),
            dynamic_rules: Vec::new(),
            nested_rules: Vec::new(),
        }
    }

    /// 定义媒体查询，例如 `.media("@media (max-width: 600px)", |s| s.width(PX(100)))`
    pub fn media<F>(mut self, query: &'static str, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nested_rules
            .push(NestedRule::Media(query, f(Style::new())));
        self
    }

    /// 定义嵌套选择器，例如 `.nest("& > div", |s| s.opacity(0.8))`
    /// 支持 "&" 占位符，若无则默认作为组合后缀（例如 ":hover"）
    pub fn nest<F>(mut self, selector: &'static str, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nested_rules
            .push(NestedRule::Selector(selector, f(Style::new())));
        self
    }

    pub fn on_hover<F>(self, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nest(":hover", f)
    }

    pub fn on_active<F>(self, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nest(":active", f)
    }

    pub fn on_focus<F>(self, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nest(":focus", f)
    }

    pub fn margin_x<V>(self, value: V) -> Self
    where
        V: IntoRx + RxValue + Clone + 'static,
        V::Value: ValidFor<MarginLeft> + ValidFor<MarginRight> + Display + Clone + Sized + 'static,
        V::RxType: RxGet<Value = V::Value> + Clone + 'static,
    {
        self.margin_left(value.clone()).margin_right(value)
    }

    pub fn margin_y<V>(self, value: V) -> Self
    where
        V: IntoRx + RxValue + Clone + 'static,
        V::Value: ValidFor<MarginTop> + ValidFor<MarginBottom> + Display + Clone + Sized + 'static,
        V::RxType: RxGet<Value = V::Value> + Clone + 'static,
    {
        self.margin_top(value.clone()).margin_bottom(value)
    }

    pub fn padding_x<V>(self, value: V) -> Self
    where
        V: IntoRx + RxValue + Clone + 'static,
        V::Value:
            ValidFor<PaddingLeft> + ValidFor<PaddingRight> + Display + Clone + Sized + 'static,
        V::RxType: RxGet<Value = V::Value> + Clone + 'static,
    {
        self.padding_left(value.clone()).padding_right(value)
    }

    pub fn padding_y<V>(self, value: V) -> Self
    where
        V: IntoRx + RxValue + Clone + 'static,
        V::Value:
            ValidFor<PaddingTop> + ValidFor<PaddingBottom> + Display + Clone + Sized + 'static,
        V::RxType: RxGet<Value = V::Value> + Clone + 'static,
    {
        self.padding_top(value.clone()).padding_bottom(value)
    }

    pub fn pseudo<F>(self, selector: &'static str, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nest(selector, f)
    }

    fn add_rule(mut self, prop: &'static str, value: StyleValue) -> Self {
        match value {
            StyleValue::Static(val_str) => {
                self.static_rules.push((prop, val_str));
            }
            StyleValue::Dynamic(getter) => {
                self.dynamic_rules.push((prop, getter));
            }
        }
        self
    }
}

pub fn sty() -> Style {
    Style::new()
}

macro_rules! generate_builder_methods {
    ($( ($snake:ident, $kebab:expr, $pascal:ident, [$($cap:ident)*]) ),*) => {
        impl Style {
            $(
                pub fn $snake<V>(self, value: V) -> Self
                where
                    V: IntoRx + RxValue + 'static,
                    V::Value: ValidFor<props::$pascal> + Display + Clone + Sized + 'static,
                    V::RxType: RxGet<Value = V::Value> + Clone + 'static,
                {
                    self.add_rule($kebab, StyleValue::from_rx(value))
                }
            )*
        }
    };
}

for_all_properties!(generate_builder_methods);

impl ApplyToDom for Style {
    fn apply(&self, el: &Element, _target: ApplyTarget) {
        self.apply_to_element(el);
    }
}

/// `Style` 编译出的产物：类名、要注入的 CSS、以及待建立的动态绑定。
///
/// 单独拆出来是为了让「生成什么 CSS」这件事能脱离 DOM 被断言——`@layer` 的
/// 归属、嵌套选择器的展开此前都只能靠读代码确认。
pub(crate) struct RenderedStyle {
    pub class_base: String,
    pub css: String,
    pub dyn_bindings: Vec<(String, DynamicValue)>,
}

impl Style {
    /// 只生成，不碰 DOM。
    pub(crate) fn render(&self) -> RenderedStyle {
        // 1. 生成稳定哈希（忽略动态值，递归所有嵌套规则）
        let mut hasher = css_hasher!();
        hash_recursive(self, &mut hasher);
        let hash_val = hasher.finish();
        let mut hash_buf = [0u8; 13];
        let hash_str = encode_base36(hash_val, &mut hash_buf);
        let class_base = format!("slx-{}", hash_str);

        // 2. 递归构造 CSS，收集所有动态绑定
        let mut css_str = String::new();
        let mut dyn_bindings = Vec::new();
        let base_sel = format!(".{}", class_base);

        generate_css_recursive(self, &base_sel, hash_str, &mut css_str, &mut dyn_bindings);

        // 3. 归入 `overrides` 层。`sty()` 是针对单个元素实例的就地覆盖，
        //    优先级理应最高；此前它**不带任何 layer**，靠「无层规则压过所有
        //    具名层」这条规范侧效达到同样效果——顺带也压过了 `global!`，
        //    而两者之间只能靠注入先后决定胜负。
        let css = if css_str.trim().is_empty() {
            String::new()
        } else {
            crate::layers::wrap(crate::layers::OVERRIDES, &css_str)
        };

        RenderedStyle {
            class_base,
            css,
            dyn_bindings,
        }
    }

    pub fn apply_to_element(&self, el: &Element) -> String {
        let RenderedStyle {
            class_base,
            css,
            dyn_bindings,
        } = self.render();

        if !css.is_empty() {
            inject_style(&class_base, &css);
        }
        let _ = el.class_list().add_1(&class_base);

        // 建立极轻量更新 Effect (只有 style.setProperty)
        //
        // 这些 Effect 是当前 owner 的子节点，owner 重跑时会被一并回收；但它们
        // **写在元素行内样式上的自定义属性**不会跟着消失。变量名带样式哈希，
        // 换一份 `Style` 就是另一批名字，旧的会永远留在 `style` 属性里。
        let mut owned_vars = Vec::with_capacity(dyn_bindings.len());
        for (var_name, getter) in dyn_bindings {
            owned_vars.push(var_name.clone());
            let el_clone = el.clone();
            Effect::new(move |prev: Option<String>| {
                let current = getter();
                if prev.as_ref() != Some(&current)
                    && let Some(style) = element_style(&el_clone)
                {
                    let _ = style.set_property(&var_name, &current);
                }
                current
            });
        }
        if !owned_vars.is_empty() {
            let el_clone = el.clone();
            on_cleanup(move || {
                if let Some(style) = element_style(&el_clone) {
                    for name in &owned_vars {
                        let _ = style.remove_property(name);
                    }
                }
            });
        }
        class_base
    }
}

fn element_style(el: &Element) -> Option<web_sys::CssStyleDeclaration> {
    el.dyn_ref::<HtmlElement>()
        .map(|e| e.style())
        .or_else(|| el.dyn_ref::<SvgElement>().map(|e| e.style()))
}

/// 递归计算样式的稳定哈希
fn hash_recursive(style: &Style, hasher: &mut CssHasher) {
    for (k, v) in &style.static_rules {
        Normalized(k).hash(hasher);
        Normalized(v).hash(hasher);
    }
    for (prop, _) in &style.dynamic_rules {
        Normalized(prop).hash(hasher);
        "dyn-val".hash(hasher); // 动态值占位
    }
    for rule in &style.nested_rules {
        match rule {
            NestedRule::Media(query, sub) => {
                "media".hash(hasher);
                Normalized(query).hash(hasher);
                hash_recursive(sub, hasher);
            }
            NestedRule::Selector(selector, sub) => {
                "selector".hash(hasher);
                Normalized(selector).hash(hasher);
                hash_recursive(sub, hasher);
            }
        }
    }
}

/// 递归生成 CSS 字符串并收集动态绑定
fn generate_css_recursive(
    style: &Style,
    base_selector: &str,
    hash_str: &str,
    css_out: &mut String,
    dyn_bindings: &mut Vec<(String, DynamicValue)>,
) {
    // 写入当前层级的规则
    if !style.static_rules.is_empty() || !style.dynamic_rules.is_empty() {
        css_out.push_str(base_selector);
        css_out.push_str(" {\n");
        for (k, v) in &style.static_rules {
            // 属性名来自注册表（`&'static str`），值可能来自用户输入：
            // 必须挡在声明边界内，否则一个 `;` 就能注入新规则
            let _ = writeln!(css_out, "  {}: {};", k, crate::escape::declaration_value(v));
        }
        for (prop, getter) in &style.dynamic_rules {
            let var_name = format!("--sb-{}-{}", hash_str, dyn_bindings.len());
            let _ = writeln!(css_out, "  {}: var({});", prop, var_name);
            dyn_bindings.push((var_name, getter.clone()));
        }
        css_out.push_str("}\n");
    }

    // 处理嵌套规则
    for rule in &style.nested_rules {
        match rule {
            NestedRule::Media(query, sub) => {
                css_out.push_str(query);
                css_out.push_str(" {\n");
                generate_css_recursive(sub, base_selector, hash_str, css_out, dyn_bindings);
                css_out.push_str("}\n");
            }
            NestedRule::Selector(selector, sub) => {
                let full_selector = if selector.contains('&') {
                    selector.replace('&', base_selector)
                } else {
                    format!("{}{}", base_selector, selector)
                };
                generate_css_recursive(sub, &full_selector, hash_str, css_out, dyn_bindings);
            }
        }
    }
}

impl ReactiveApply for Style {
    fn apply_to_dom(rx: Rx<Self, RxValueKind>, el: Element, _target: ApplyTarget) {
        let el = el.clone();
        Effect::new(move |prev_class: Option<String>| {
            if let Some(c) = &prev_class {
                let _ = el.class_list().remove_1(c);
            }
            let style = rx.get();
            style.apply_to_element(&el)
        });
    }
}

impl From<Option<Style>> for Style {
    fn from(opt: Option<Style>) -> Self {
        opt.unwrap_or_default()
    }
}

impl IntoStorable for Style {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers;
    use crate::types::{hex, px};

    fn css_of(style: Style) -> String {
        style.render().css
    }

    /// 报告 P2-3：`sty()` 的产出此前**不带任何 layer**，靠「无层规则压过所有
    /// 具名层」的规范侧效获得最高优先级——顺带压过了 `global!`，而两者之间
    /// 只能靠注入先后决定胜负。
    #[test]
    fn sty_lands_in_the_overrides_layer() {
        let css = css_of(Style::new().color(hex("#fff")));
        assert!(
            css.starts_with(&format!("@layer {} {{", layers::OVERRIDES)),
            "{css}"
        );
        assert!(css.contains("color: #fff;"), "{css}");
    }

    /// 空样式不该注入一个空的 layer 块
    #[test]
    fn an_empty_style_produces_no_css_at_all() {
        assert_eq!(css_of(Style::new()), "");
    }

    /// 类名只由静态结构决定：同样的声明必须给出同一个类名，
    /// 否则每次渲染都会往静态表里塞一份新副本
    #[test]
    fn the_class_name_is_stable_across_renders() {
        let a = Style::new().color(hex("#fff")).width(px(10)).render();
        let b = Style::new().color(hex("#fff")).width(px(10)).render();
        assert_eq!(a.class_base, b.class_base);
        assert!(a.class_base.starts_with("slx-"));
    }

    /// 值来自用户输入时必须挡在声明边界内（报告 P0-8 的回归）。
    ///
    /// 产物里只该有两层花括号：`@layer` 块与那条规则本身。多出来的就是被
    /// 用户字符串撑开的新规则。
    #[test]
    fn a_static_value_cannot_break_out_of_its_declaration() {
        let css = css_of(Style::new().grid_template_areas("red; } body { display: none"));
        assert_eq!(css.matches('{').count(), 2, "{css}");
        assert_eq!(css.matches('}').count(), 2, "{css}");
        assert_eq!(css.matches(';').count(), 1, "{css}");
    }

    /// 嵌套选择器：带 `&` 的替换到位，不带 `&` 的按后缀拼接
    #[test]
    fn nested_selectors_expand_against_the_base_class() {
        let rendered = Style::new()
            .nest("& > div", |s| s.color(hex("#000")))
            .render();
        let base = format!(".{}", rendered.class_base);
        assert!(
            rendered.css.contains(&format!("{base} > div {{")),
            "{}",
            rendered.css
        );
    }

    /// `apply_to_element` 每次调用都为每个动态绑定新建一个 `Effect`，而
    /// `ReactiveApply::apply_to_dom` 会在一个外层 `Effect` 里反复调用它。
    /// 这里成立的前提是：内层 `Effect` 是外层的子节点，外层重跑时随之回收。
    ///
    /// 这条不变量在 `silex_reactivity` 里（`run_effect` → `run_cleanups` →
    /// `dispose_node_internal(child)`），但 `builder.rs` 依赖它，所以在这里
    /// 钉一根桩：哪天所有权模型变了，先坏在这儿而不是坏成线上的 Effect 泄漏。
    #[test]
    fn inner_effects_are_reclaimed_when_the_outer_effect_reruns() {
        use silex_core::{reactivity::RwSignal, traits::RxWrite};
        use std::{cell::Cell, rc::Rc};

        let outer = RwSignal::new(0);
        let inner_dep = RwSignal::new(0);
        let inner_runs = Rc::new(Cell::new(0));

        let counter = inner_runs.clone();
        Effect::new(move |_| {
            outer.get();
            let counter = counter.clone();
            Effect::new(move |_| {
                inner_dep.get();
                counter.set(counter.get() + 1);
            });
        });

        assert_eq!(inner_runs.get(), 1, "首轮内层 Effect 跑一次");
        outer.set(1);
        assert_eq!(inner_runs.get(), 2, "外层重跑，新内层 Effect 跑一次");
        inner_dep.set(1);
        assert_eq!(
            inner_runs.get(),
            3,
            "只有存活的那个内层 Effect 响应；上一轮的若没回收会多跑一次"
        );
    }

    /// 动态值走行内 CSS 变量，规则里只留一个 `var()` 引用
    #[test]
    fn dynamic_values_become_a_css_variable_reference() {
        let signal = silex_core::reactivity::RwSignal::new(px(1));
        let rendered = Style::new().width(signal).render();
        assert_eq!(rendered.dyn_bindings.len(), 1);
        let var_name = &rendered.dyn_bindings[0].0;
        assert!(var_name.starts_with("--sb-"), "{var_name}");
        assert!(rendered.css.contains(&format!("width: var({var_name});")));
    }
}
