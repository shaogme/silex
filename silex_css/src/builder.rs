use crate::types::{ValidFor, props};
use silex_core::traits::{Get, IntoSignal, With};
use silex_dom::attribute::{ApplyTarget, ApplyToDom, IntoStorable};
use std::fmt::Display;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use wasm_bindgen::JsCast;

pub(crate) type DynamicValue = Rc<dyn Fn() -> String>;
pub(crate) type StaticRule = (&'static str, String);
pub(crate) type DynamicRule = (&'static str, DynamicValue);
pub(crate) type PseudoRule = (&'static str, Style);

#[derive(Clone)]
pub struct Style {
    pub(crate) static_rules: Vec<StaticRule>,
    pub(crate) dynamic_rules: Vec<DynamicRule>,
    pub(crate) pseudo_rules: Vec<PseudoRule>,
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
            pseudo_rules: Vec::new(),
        }
    }

    pub fn on_hover<F>(mut self, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.pseudo_rules.push((":hover", f(Style::new())));
        self
    }

    pub fn on_active<F>(mut self, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.pseudo_rules.push((":active", f(Style::new())));
        self
    }

    pub fn on_focus<F>(mut self, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.pseudo_rules.push((":focus", f(Style::new())));
        self
    }

    pub fn pseudo<F>(mut self, selector: &'static str, f: F) -> Self
    where
        F: FnOnce(Style) -> Style,
    {
        self.pseudo_rules.push((selector, f(Style::new())));
        self
    }

    /// 内部通用方法：添加一条 CSS 规则
    fn add_rule<V, ValType, P>(mut self, prop: &'static str, value: V) -> Self
    where
        V: IntoSignal<Value = ValType> + 'static,
        ValType: ValidFor<P> + Display + Clone + 'static,
        <V as IntoSignal>::Signal: Get + 'static,
        <<V as IntoSignal>::Signal as With>::Value: Display,
    {
        if value.is_constant_value() {
            let signal = value.into_signal();
            let val_str = format!("{}", signal.get());
            self.static_rules.push((prop, val_str));
        } else {
            let signal = value.into_signal();
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
                pub fn $snake<V, ValType>(self, value: V) -> Self
                where
                    V: IntoSignal<Value = ValType> + 'static,
                    ValType: ValidFor<props::$pascal> + Display + Clone + 'static,
                    <V as IntoSignal>::Signal: Get + 'static,
                    <<V as IntoSignal>::Signal as With>::Value: Display,
                {
                    self.add_rule::<V, ValType, props::$pascal>($kebab, value)
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
        // 1. 生成稳定哈希（忽略动态值，仅对选择器和属性名哈希）
        let mut hasher = DefaultHasher::new();
        for (k, v) in &self.static_rules {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        for (prop, _) in &self.dynamic_rules {
            prop.hash(&mut hasher);
            "dyn-val".hash(&mut hasher); // 动态值占位
        }
        for (pseudo, style) in &self.pseudo_rules {
            pseudo.hash(&mut hasher);
            for (k, v) in &style.static_rules {
                k.hash(&mut hasher);
                v.hash(&mut hasher);
            }
            for (prop, _) in &style.dynamic_rules {
                prop.hash(&mut hasher);
                "dyn-val".hash(&mut hasher);
            }
        }
        let hash_val = hasher.finish();
        let class_base = format!("slx-bldr-{:x}", hash_val);

        // 2. 构造静态 CSS，动态值使用变量占位
        let mut css_str = String::new();
        let mut dyn_bindings = Vec::new();

        css_str.push_str(&format!(".{} {{\n", class_base));
        for (k, v) in &self.static_rules {
            css_str.push_str(&format!("  {}: {};\n", k, v));
        }
        for (i, (prop, getter)) in self.dynamic_rules.into_iter().enumerate() {
            let var_name = format!("--sb-{:x}-{}", hash_val, i);
            css_str.push_str(&format!("  {}: var({});\n", prop, var_name));
            dyn_bindings.push((var_name, getter));
        }
        css_str.push_str("}\n");

        let mut dyn_idx = dyn_bindings.len();
        for (pseudo, style) in self.pseudo_rules {
            css_str.push_str(&format!(".{}{} {{\n", class_base, pseudo));
            for (k, v) in style.static_rules {
                css_str.push_str(&format!("  {}: {};\n", k, v));
            }
            for (prop, getter) in style.dynamic_rules {
                let var_name = format!("--sb-{:x}-{}", hash_val, dyn_idx);
                css_str.push_str(&format!("  {}: var({});\n", prop, var_name));
                dyn_bindings.push((var_name, getter));
                dyn_idx += 1;
            }
            css_str.push_str("}\n");
        }

        // 3. 注入样式并添加类名
        crate::inject_style(&class_base, &css_str);
        let _ = el.class_list().add_1(&class_base);

        // 4. 建立极轻量更新 Effect (只有 style.setProperty)
        for (var_name, getter) in dyn_bindings {
            let el_clone = el.clone();
            silex_core::reactivity::Effect::new(move |_| {
                if let Some(style) = el_clone
                    .dyn_ref::<web_sys::HtmlElement>()
                    .map(|e| e.style())
                    .or_else(|| el_clone.dyn_ref::<web_sys::SvgElement>().map(|e| e.style()))
                {
                    let _ = style.set_property(&var_name, &getter());
                }
            });
        }
        class_base
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
