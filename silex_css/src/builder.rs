use crate::types::{ValidFor, props};
use silex_core::traits::{IntoRx, RxGet, RxInternal, RxRead, RxValue};
use silex_dom::attribute::{ApplyTarget, ApplyToDom, IntoStorable};
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use wasm_bindgen::JsCast;

pub(crate) type DynamicValue = Rc<dyn Fn() -> String>;
pub(crate) type StaticRule = (&'static str, String);
pub(crate) type DynamicRule = (&'static str, DynamicValue);

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

    pub fn pseudo<F>(self, selector: &'static str, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.nest(selector, f)
    }

    fn add_rule<V, P>(mut self, prop: &'static str, value: V) -> Self
    where
        V: IntoRx + RxValue + 'static,
        V::Value: Display + ValidFor<P> + Clone + Sized,
        V::RxType: RxRead + RxValue<Value = V::Value> + Clone + 'static,
        for<'a> <V::RxType as RxInternal>::ReadOutput<'a>: std::ops::Deref<Target = V::Value>,
    {
        if value.is_constant() {
            let signal = value.into_rx();
            let val_str = format!("{}", signal.get());
            self.static_rules.push((prop, val_str));
        } else {
            let signal = value.into_rx();
            self.dynamic_rules
                .push((prop, Rc::new(move || format!("{}", signal.get()))));
        }
        self
    }
}

pub fn sty() -> Style {
    Style::new()
}

macro_rules! generate_builder_methods {
    ($( ($snake:ident, $kebab:expr, $pascal:ident, $group:ident) ),*) => {
        impl Style {
            $(
                pub fn $snake<V>(self, value: V) -> Self
                where
                    V: IntoRx + RxValue + 'static,
                    V::Value: ValidFor<props::$pascal> + Display + Clone + Sized + 'static,
                    V::RxType: RxRead + RxValue<Value = V::Value> + Clone + 'static,
                    for<'a> <V::RxType as RxInternal>::ReadOutput<'a>: std::ops::Deref<Target = V::Value>,
                {
                    self.add_rule::<V, props::$pascal>($kebab, value)
                }
            )*
        }
    };
}

crate::for_all_properties!(generate_builder_methods);

impl ApplyToDom for Style {
    fn apply(self, el: &web_sys::Element, _target: ApplyTarget) {
        self.apply_to_element(el);
    }
}

impl Style {
    pub fn apply_to_element(self, el: &web_sys::Element) -> String {
        // 1. 生成稳定哈希（忽略动态值，递归所有嵌套规则）
        let mut hasher = silex_hash::css::CssHasher::new();
        hash_recursive(&self, &mut hasher);
        let hash_val = hasher.finish();
        let mut hash_buf = [0u8; 13];
        let hash_str = silex_hash::css::encode_base36(hash_val, &mut hash_buf);
        let class_base = format!("slx-{}", hash_str);

        // 2. 递归构造 CSS，收集所有动态绑定
        let mut css_str = String::new();
        let mut dyn_bindings = Vec::new();
        let base_sel = format!(".{}", class_base);

        generate_css_recursive(&self, &base_sel, hash_str, &mut css_str, &mut dyn_bindings);

        // 3. 注入样式并添加类名
        crate::inject_style(&class_base, &css_str);
        let _ = el.class_list().add_1(&class_base);

        // 4. 建立极轻量更新 Effect (只有 style.setProperty)
        for (var_name, getter) in dyn_bindings {
            let el_clone = el.clone();
            silex_core::reactivity::Effect::new(move |prev: Option<String>| {
                let current = getter();
                if prev.as_ref() != Some(&current)
                    && let Some(style) = el_clone
                        .dyn_ref::<web_sys::HtmlElement>()
                        .map(|e| e.style())
                        .or_else(|| el_clone.dyn_ref::<web_sys::SvgElement>().map(|e| e.style()))
                {
                    let _ = style.set_property(&var_name, &current);
                }
                current
            });
        }
        class_base
    }
}

/// 递归计算样式的稳定哈希
fn hash_recursive(style: &Style, hasher: &mut silex_hash::css::CssHasher) {
    for (k, v) in &style.static_rules {
        silex_hash::css::Normalized(k).hash(hasher);
        silex_hash::css::Normalized(v).hash(hasher);
    }
    for (prop, _) in &style.dynamic_rules {
        silex_hash::css::Normalized(prop).hash(hasher);
        "dyn-val".hash(hasher); // 动态值占位
    }
    for rule in &style.nested_rules {
        match rule {
            NestedRule::Media(query, sub) => {
                "media".hash(hasher);
                silex_hash::css::Normalized(query).hash(hasher);
                hash_recursive(sub, hasher);
            }
            NestedRule::Selector(selector, sub) => {
                "selector".hash(hasher);
                silex_hash::css::Normalized(selector).hash(hasher);
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
            css_out.push_str(&format!("  {}: {};\n", k, v));
        }
        for (prop, getter) in &style.dynamic_rules {
            let var_name = format!("--sb-{}-{}", hash_str, dyn_bindings.len());
            css_out.push_str(&format!("  {}: var({});\n", prop, var_name));
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

impl silex_dom::attribute::ReactiveApply for Style {
    fn apply_to_dom(
        f: impl Fn() -> Self + 'static,
        el: web_sys::Element,
        _target: silex_dom::attribute::OwnedApplyTarget,
    ) {
        let el = el.clone();
        silex_core::reactivity::Effect::new(move |prev_class: Option<String>| {
            if let Some(c) = prev_class {
                let _ = el.class_list().remove_1(&c);
            }
            let style = f();
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
