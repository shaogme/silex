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
    for write in var_writes(entries, prev) {
        match write {
            VarWrite::Set(name, value) => {
                let _ = style.set_property(name, value);
            }
            VarWrite::Remove(name) => {
                let _ = style.remove_property(name);
            }
        }
    }
    entries.iter().map(|(_, v)| v.clone()).collect()
}

/// 一次变量写入。
///
/// 「写什么」与「写到哪」分开，是为了让前者能脱离 `CssStyleDeclaration`（也就
/// 是脱离浏览器）被断言——这段 diff 逻辑此前在三个地方各抄了一遍，一个测试也
/// 没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VarWrite<'a> {
    Set(&'a str, &'a str),
    Remove(&'a str),
}

/// 与上一轮取值比较，给出这一轮真正需要落到 DOM 上的写入。
///
/// 没变的变量不写——每个变量的 `setProperty` 都会让浏览器重算受它影响的那棵
/// 子树，主题里几十个变量一起重写是实打实的开销。
fn var_writes<'a>(
    entries: &'a [(&'static str, Option<String>)],
    prev: Option<&Vec<Option<String>>>,
) -> Vec<VarWrite<'a>> {
    let mut out = Vec::new();
    for (i, (name, value)) in entries.iter().enumerate() {
        if prev.and_then(|p| p.get(i)) == Some(value) {
            continue;
        }
        out.push(match value {
            Some(v) => VarWrite::Set(name, v),
            None => VarWrite::Remove(name),
        });
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(&'static str, Option<&str>)]) -> Vec<(&'static str, Option<String>)> {
        pairs
            .iter()
            .map(|(n, v)| (*n, v.map(str::to_string)))
            .collect()
    }

    fn prev_of(pairs: &[Option<&str>]) -> Vec<Option<String>> {
        pairs.iter().map(|v| v.map(str::to_string)).collect()
    }

    /// 首轮没有上一轮取值，所有变量都要写
    #[test]
    fn the_first_round_writes_everything() {
        let e = entries(&[("--a", Some("1")), ("--b", Some("2"))]);
        assert_eq!(
            var_writes(&e, None),
            vec![VarWrite::Set("--a", "1"), VarWrite::Set("--b", "2")]
        );
    }

    /// 没变的变量不写：每个 `setProperty` 都会让浏览器重算受影响的子树
    #[test]
    fn unchanged_variables_are_not_rewritten() {
        let e = entries(&[("--a", Some("1")), ("--b", Some("9"))]);
        let prev = prev_of(&[Some("1"), Some("2")]);
        assert_eq!(var_writes(&e, Some(&prev)), vec![VarWrite::Set("--b", "9")]);
    }

    /// `None` 是「移除」，不是「设成空串」——设成空串会触发
    /// *invalid at computed-value time*，取到的是初始值而不是继承来的值
    #[test]
    fn a_none_value_removes_the_variable() {
        let e = entries(&[("--a", None)]);
        let prev = prev_of(&[Some("1")]);
        assert_eq!(var_writes(&e, Some(&prev)), vec![VarWrite::Remove("--a")]);
    }

    /// 上一轮就已经是「不存在」，这一轮还是——不必再 remove 一次
    #[test]
    fn an_already_absent_variable_is_left_alone() {
        let e = entries(&[("--a", None)]);
        let prev = prev_of(&[None]);
        assert!(var_writes(&e, Some(&prev)).is_empty());
    }

    /// 变量数量变多时，多出来的那几个必须被写进去。
    ///
    /// 这一条盯的是「静默截断」：此前两处 diff 用的是
    /// `names.iter().zip(values.iter())`，两个列表长度不一致时短的那个说了算，
    /// 多出来的变量不会有任何提示。
    #[test]
    fn a_longer_entry_list_is_not_truncated_against_a_shorter_previous_round() {
        let e = entries(&[("--a", Some("1")), ("--b", Some("2")), ("--c", Some("3"))]);
        let prev = prev_of(&[Some("1")]);
        assert_eq!(
            var_writes(&e, Some(&prev)),
            vec![VarWrite::Set("--b", "2"), VarWrite::Set("--c", "3")]
        );
    }

    /// 变量名与取值的配对必须一一对应
    #[test]
    fn theme_entries_pairs_names_with_values_in_order() {
        struct T;
        impl Display for T {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("--a:1;--b:2;")
            }
        }
        impl ThemeToCss for T {
            fn get_variable_values(&self) -> Vec<String> {
                vec!["1".into(), "2".into()]
            }
            fn get_variable_names() -> &'static [&'static str] {
                &["--a", "--b"]
            }
        }
        assert_eq!(
            theme_entries(&T),
            entries(&[("--a", Some("1")), ("--b", Some("2"))])
        );
    }

    /// 名字与取值数量对不上时，debug 构建下必须炸出来而不是静默丢弃
    #[test]
    #[should_panic(expected = "变量名与取值数量不一致")]
    fn a_mismatched_theme_is_caught_in_debug_builds() {
        struct Broken;
        impl Display for Broken {
            fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Ok(())
            }
        }
        impl ThemeToCss for Broken {
            fn get_variable_values(&self) -> Vec<String> {
                vec!["1".into()]
            }
            fn get_variable_names() -> &'static [&'static str] {
                &["--a", "--b"]
            }
        }
        let _ = theme_entries(&Broken);
    }
}
