use crate::runtime::DynamicStyleManager;
use silex_core::prelude::*;
use silex_dom::attribute::{ApplyTarget, ApplyToDom, IntoStorable};
use std::{cell::RefCell, fmt::Display, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{CssStyleDeclaration, Element, HtmlElement, SvgElement};

/// A trait that every Silex Theme must implement.
/// This allows the `styled!` macro to perform compile-time type checks.
/// Usually implemented via the `theme!` macro.
pub trait ThemeType {}

/// 主题到 CSS 变量的映射。
///
/// `Display` 给出 `--name: value;` 的拼接结果（`set_global_theme` 用它拼
/// `:root{}` 规则）。此前这里还有一个 `to_css_variables()`，它的唯一消费者
/// 就是宏生成的 `Display` 实现本身——一个只被自己调用的 trait 方法。
pub trait ThemeToCss: Display {
    fn get_variable_values(&self) -> Vec<String>;
    fn get_variable_names() -> &'static [&'static str];
}

/// 一组 CSS 变量的增量写入，返回本轮的取值供下一轮比较。
///
/// `None` 表示移除该变量（让它回到继承来的值）。
///
/// 此前 `ThemeVariables`、`set_global_theme`、`ThemePatchVariables` 三处各写了
/// 一份几乎相同的「取值 → 与上一轮比较 → setProperty」；前两处还是
/// `names.iter().zip(values.iter())`——两个列表长度不一致时**静默截断**，
/// 少写的那几个变量不会有任何提示。
fn apply_var_diff(
    style: &CssStyleDeclaration,
    entries: &[(&'static str, Option<String>)],
    prev: Option<&Vec<Option<String>>>,
) -> Vec<Option<String>> {
    let mut current = Vec::with_capacity(entries.len());
    for (i, (name, value)) in entries.iter().enumerate() {
        if prev.and_then(|p| p.get(i)) != Some(value) {
            match value {
                Some(v) => {
                    let _ = style.set_property(name, v);
                }
                None => {
                    let _ = style.remove_property(name);
                }
            }
        }
        current.push(value.clone());
    }
    current
}

/// 元素上可写的 style 对象（HTML 与 SVG 两条路）。
fn element_style(el: &Element) -> Option<CssStyleDeclaration> {
    el.dyn_ref::<HtmlElement>()
        .map(|e| e.style())
        .or_else(|| el.dyn_ref::<SvgElement>().map(|e| e.style()))
}

/// 把主题的当前取值配成 `(变量名, 值)`。
fn theme_entries<T: ThemeToCss>(theme: &T) -> Vec<(&'static str, Option<String>)> {
    let names = T::get_variable_names();
    let values = theme.get_variable_values();
    debug_assert_eq!(
        names.len(),
        values.len(),
        "主题的变量名与取值数量不一致：`get_variable_names()` 与 \
         `get_variable_values()` 必须一一对应，否则多出来的那几个会被静默丢掉"
    );
    names
        .iter()
        .zip(values)
        .map(|(name, value)| (*name, Some(value)))
        .collect()
}

/// Helper that applies theme variables to any element without an extra wrapper.
/// Usage: `div(children).apply(theme_variables(theme))`
pub fn theme_variables<T>(theme: impl IntoSignal<Value = T> + 'static) -> ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    ThemeVariables(theme.into_signal())
}

/// A structure that can be applied to a DOM element to inject theme variables.
pub struct ThemeVariables<T>(pub Signal<T>);

impl<T> ApplyToDom for ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    fn apply(&self, el: &Element, _target: ApplyTarget) {
        let theme = self.0;
        let el = el.clone();
        Effect::new(move |prev: Option<Vec<Option<String>>>| {
            let Some(style) = element_style(&el) else {
                return Vec::new();
            };
            let entries = theme_entries(&theme.get());
            apply_var_diff(&style, &entries, prev.as_ref())
        });
    }
}

impl<T> IntoStorable for ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

/// 全局主题下 `:root{}` 规则所用的样式表 id。
const GLOBAL_THEME_STYLE_ID: &str = "slx-global-theme";

/// Sets a global theme that applies to the entire document (:root).
///
/// 此前这里写的是 `documentElement` 的**行内 style**，而不是一条 `:root{}` 规则：
///
/// - CSP 的 `style-src` 没开 `unsafe-inline` 时直接失效；
/// - 行内优先级压过作者样式表里任何 `:root` 定义，主题就没法被局部覆盖了。
///
/// 现在走和其他动态样式同一条路：一张构造式样式表，内容是一条 `:root{}` 规则。
/// 它不属于任何级联层（无层规则优先级最高），所以仍然压得住 `base` 里的
/// 默认变量——但这是**规则之间**的较量，而不是行内样式的碾压。
pub fn set_global_theme<T>(theme: impl IntoSignal<Value = T> + 'static)
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    let signal = theme.into_signal();

    let manager = Rc::new(RefCell::new(Some(DynamicStyleManager::new())));
    let cleanup = manager.clone();
    on_cleanup(move || {
        if let Ok(mut opt) = cleanup.try_borrow_mut() {
            let _ = opt.take();
        }
    });

    Effect::new(move |prev: Option<String>| {
        let theme_val = signal.get();
        debug_assert_eq!(
            T::get_variable_names().len(),
            theme_val.get_variable_values().len(),
            "主题的变量名与取值数量不一致"
        );
        let css = format!(":root{{{}}}", theme_val);
        if prev.as_deref() != Some(css.as_str())
            && let Ok(mut opt) = manager.try_borrow_mut()
            && let Some(m) = opt.as_mut()
        {
            m.update(GLOBAL_THEME_STYLE_ID, &css);
        }
        css
    });
}

/// A trait for theme patches that only override a subset of variables.
pub trait ThemePatchToCss {
    /// Returns a list of (variable_name, value).
    /// If the value is None, the variable should be removed from the local element style (enabling inheritance).
    fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)>;
}

/// Helper that applies a theme patch to an element.
/// This allows for granular overrides while relying on CSS variable inheritance for the rest.
pub fn theme_patch<P>(patch: impl IntoSignal<Value = P> + 'static) -> ThemePatchVariables<P>
where
    P: ThemePatchToCss + RxCloneData + 'static,
{
    ThemePatchVariables(patch.into_signal())
}

/// A structure that can be applied to a DOM element to inject theme patch variables.
pub struct ThemePatchVariables<P>(pub Signal<P>);

impl<P> ApplyToDom for ThemePatchVariables<P>
where
    P: ThemePatchToCss + RxCloneData + 'static,
{
    fn apply(&self, el: &Element, _target: ApplyTarget) {
        let patch = self.0;
        let el = el.clone();
        Effect::new(move |prev: Option<Vec<Option<String>>>| {
            let Some(style) = element_style(&el) else {
                return Vec::new();
            };
            let entries = patch.get().get_patch_entries();
            apply_var_diff(&style, &entries, prev.as_ref())
        });
    }
}

impl<P> IntoStorable for ThemePatchVariables<P>
where
    P: ThemePatchToCss + RxCloneData + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
