use crate::{
    runtime::{DynamicStyleManager, dynamic::unique_dynamic_style_id},
    source::{CssSource, IntoCssSource},
};
use silex_core::{RuntimeInputs, SilexError, SilexResult};
use silex_dom::{
    attribute::{ApplyTarget, ApplyToDom, AttrOp, IntoStorable},
    view::{ViewOwner, ViewOwnerToken},
};
use std::{
    cell::RefCell,
    fmt::{Display, Write},
    rc::Rc,
};
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
    prev: Option<&Vec<(&'static str, Option<String>)>>,
) -> SilexResult<Vec<(&'static str, Option<String>)>> {
    for write in var_writes(entries, prev) {
        match write {
            VarWrite::Set(name, value) => {
                style.set_property(name, value)?;
            }
            VarWrite::Remove(name) => {
                style.remove_property(name)?;
            }
        }
    }
    Ok(entries.to_vec())
}

/// 一次变量写入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VarWrite<'a> {
    Set(&'a str, &'a str),
    Remove(&'a str),
}

/// 与上一轮取值比较，给出这一轮真正需要落到 DOM 上的写入。
fn var_writes<'a>(
    entries: &'a [(&'static str, Option<String>)],
    prev: Option<&Vec<(&'static str, Option<String>)>>,
) -> Vec<VarWrite<'a>> {
    let mut out = Vec::new();
    for (name, value) in entries {
        if prev
            .and_then(|p| p.iter().find(|(old_name, _)| old_name == name))
            .map(|(_, old_value)| old_value)
            == Some(value)
        {
            continue;
        }
        out.push(match value {
            Some(v) => VarWrite::Set(name, v),
            None => VarWrite::Remove(name),
        });
    }
    if let Some(prev) = prev {
        for (old_name, _) in prev {
            if !entries.iter().any(|(name, _)| name == old_name) {
                out.push(VarWrite::Remove(old_name));
            }
        }
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
fn theme_entries<T: ThemeToCss>(theme: &T) -> SilexResult<Vec<(&'static str, Option<String>)>> {
    let names = T::get_variable_names();
    let values = theme.get_variable_values();
    if names.len() != values.len() {
        return Err(SilexError::Framework("主题的变量名与取值数量不一致".into()));
    }
    Ok(names
        .iter()
        .zip(values)
        .map(|(name, value)| (*name, Some(value)))
        .collect())
}

fn source_inputs<'scope, T: 'scope>(source: &CssSource<'scope, T>) -> RuntimeInputs {
    match source {
        CssSource::Static(_) => RuntimeInputs::new(),
        CssSource::Reactive(rx) => rx.runtime_inputs(),
    }
}

/// Helper that applies theme variables to any element without an extra wrapper.
/// Usage: `div(children).apply(theme_variables(theme))`
pub fn theme_variables<'scope, S>(theme: S) -> ThemeVariables<'scope, S::Value>
where
    S: IntoCssSource<'scope>,
    S::Value: ThemeType + ThemeToCss + Clone + 'scope,
{
    ThemeVariables(theme.into_css_source())
}

/// A structure that can be applied to a DOM element to inject theme variables.
pub struct ThemeVariables<'scope, T>(pub CssSource<'scope, T>);

impl<'scope, T> ApplyToDom<'scope> for ThemeVariables<'scope, T>
where
    T: ThemeType + ThemeToCss + Clone + 'scope,
{
    fn apply(
        &self,
        el: &Element,
        _target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        let theme = self.0.clone();
        let inputs = source_inputs(&theme);
        owner.validate_inputs(&inputs)?;
        let el = el.clone();
        let effect_el = el.clone();
        let previous = Rc::new(RefCell::new(None::<Vec<(&'static str, Option<String>)>>));
        let previous_for_effect = previous.clone();
        let error_handler = owner.error_handler();
        owner.effect_from(
            inputs,
            Box::new(move || -> SilexResult<()> {
                let theme = match &theme {
                    CssSource::Static(theme) => theme.clone(),
                    CssSource::Reactive(rx) => rx.try_get()?,
                };
                let Some(style) = element_style(&effect_el) else {
                    return Err(SilexError::Dom(
                        "element does not expose a style declaration".into(),
                    ));
                };
                let entries = theme_entries(&theme)?;
                let next = {
                    let previous = previous_for_effect.borrow();
                    apply_var_diff(&style, &entries, previous.as_ref())?
                };
                *previous_for_effect.borrow_mut() = Some(next);
                Ok(())
            }),
            error_handler,
        )?;
        let names = T::get_variable_names().to_vec();
        let el_clone = el.clone();
        owner.on_cleanup(
            Box::new(move || -> SilexResult<()> {
                let mut first_error = None;
                if let Some(style) = element_style(&el_clone) {
                    for name in &names {
                        if let Err(error) = style.remove_property(name) {
                            first_error.get_or_insert_with(|| SilexError::from(error));
                        }
                    }
                }
                first_error.map_or(Ok(()), Err)
            }),
            error_handler,
        )?;
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        let inputs = source_inputs(&self.0);
        AttrOp::custom_with_inputs(inputs, move |el, owner| {
            self.apply(el, ApplyTarget::Apply, owner)
        })
    }
}

impl<'scope, T> IntoStorable<'scope> for ThemeVariables<'scope, T>
where
    T: ThemeType + ThemeToCss + Clone + 'scope,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

/// 全局主题下 `:root{}` 规则所用的样式表 id。
/// Sets a global theme that applies to the entire document (:root).
pub fn set_global_theme<'scope, S>(owner: &dyn ViewOwner<'scope>, theme: S) -> SilexResult<()>
where
    S: IntoCssSource<'scope>,
    S::Value: ThemeType + ThemeToCss + Clone + 'scope,
{
    let source = theme.into_css_source();
    let inputs = source_inputs(&source);
    owner.validate_inputs(&inputs)?;
    let manager = Rc::new(DynamicStyleManager::new());
    let manager_for_effect = manager.clone();
    let style_id = unique_dynamic_style_id("slx-global-theme");
    let previous = Rc::new(RefCell::new(None::<String>));
    let previous_for_effect = previous.clone();
    let error_handler = owner.token().error_handler();
    owner
        .effect_from(
            inputs,
            Box::new(move || -> SilexResult<()> {
                let theme = match &source {
                    CssSource::Static(theme) => theme.clone(),
                    CssSource::Reactive(rx) => rx.try_get()?,
                };
                let css = global_theme_css(&theme)?;
                if previous_for_effect.borrow().as_deref() != Some(css.as_str())
                    && !manager_for_effect.update(&style_id, &css)
                {
                    return Err(SilexError::Dom("无法更新全局主题样式表".into()));
                }
                *previous_for_effect.borrow_mut() = Some(css);
                Ok(())
            }),
            error_handler,
        )
        .inspect_err(|_| manager.dispose())?;
    let manager_for_cleanup = manager.clone();
    owner.on_cleanup(
        Box::new(move || {
            manager_for_cleanup.dispose();
            Ok(())
        }),
        error_handler,
    )?;
    Ok(())
}

fn global_theme_css<T: ThemeToCss>(theme: &T) -> SilexResult<String> {
    let entries = theme_entries(theme)?;
    let mut css = String::from(":root{");
    for (name, value) in entries {
        if let Some(value) = value {
            let _ = writeln!(
                css,
                "{}:{};",
                crate::escape::property_name(name),
                crate::escape::declaration_value(&value)
            );
        }
    }
    css.push('}');
    Ok(css)
}

/// A trait for theme patches that only override a subset of variables.
pub trait ThemePatchToCss {
    /// Returns a list of (variable_name, value).
    /// If the value is None, the variable should be removed from the local element style (enabling inheritance).
    fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)>;
}

/// Helper that applies a theme patch to an element.
/// This allows for granular overrides while relying on CSS variable inheritance for the rest.
pub fn theme_patch<'scope, S>(patch: S) -> ThemePatchVariables<'scope, S::Value>
where
    S: IntoCssSource<'scope>,
    S::Value: ThemePatchToCss + Clone + 'scope,
{
    ThemePatchVariables(patch.into_css_source())
}

/// A structure that can be applied to a DOM element to inject theme patch variables.
pub struct ThemePatchVariables<'scope, P>(pub CssSource<'scope, P>);

impl<'scope, P> ApplyToDom<'scope> for ThemePatchVariables<'scope, P>
where
    P: ThemePatchToCss + Clone + 'scope,
{
    fn apply(
        &self,
        el: &Element,
        _target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        let patch = self.0.clone();
        let inputs = source_inputs(&patch);
        owner.validate_inputs(&inputs)?;
        let el = el.clone();
        let effect_el = el.clone();
        let previous = Rc::new(RefCell::new(None::<Vec<(&'static str, Option<String>)>>));
        let names = Rc::new(RefCell::new(Vec::<&'static str>::new()));
        let previous_for_effect = previous.clone();
        let names_for_effect = names.clone();
        let error_handler = owner.error_handler();
        owner.effect_from(
            inputs,
            Box::new(move || -> SilexResult<()> {
                let patch = match &patch {
                    CssSource::Static(patch) => patch.clone(),
                    CssSource::Reactive(rx) => rx.try_get()?,
                };
                let entries = patch.get_patch_entries();
                {
                    let mut names = names_for_effect.borrow_mut();
                    for (name, _) in &entries {
                        if !names.contains(name) {
                            names.push(*name);
                        }
                    }
                }
                let Some(style) = element_style(&effect_el) else {
                    return Err(SilexError::Dom(
                        "element does not expose a style declaration".into(),
                    ));
                };
                let next = {
                    let previous = previous_for_effect.borrow();
                    apply_var_diff(&style, &entries, previous.as_ref())?
                };
                *previous_for_effect.borrow_mut() = Some(next);
                Ok(())
            }),
            error_handler,
        )?;
        let names_for_cleanup = names.clone();
        let el_clone = el.clone();
        owner.on_cleanup(
            Box::new(move || -> SilexResult<()> {
                let mut first_error = None;
                if let Some(style) = element_style(&el_clone) {
                    for name in names_for_cleanup.borrow().iter() {
                        if let Err(error) = style.remove_property(name) {
                            first_error.get_or_insert_with(|| SilexError::from(error));
                        }
                    }
                }
                first_error.map_or(Ok(()), Err)
            }),
            error_handler,
        )?;
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        let inputs = source_inputs(&self.0);
        AttrOp::custom_with_inputs(inputs, move |el, owner| {
            self.apply(el, ApplyTarget::Apply, owner)
        })
    }
}

impl<'scope, P> IntoStorable<'scope> for ThemePatchVariables<'scope, P>
where
    P: ThemePatchToCss + Clone + 'scope,
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

    fn prev_of(pairs: &[(&'static str, Option<&str>)]) -> Vec<(&'static str, Option<String>)> {
        pairs
            .iter()
            .map(|(name, value)| (*name, value.map(str::to_string)))
            .collect()
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
        let prev = prev_of(&[("--a", Some("1")), ("--b", Some("2"))]);
        assert_eq!(var_writes(&e, Some(&prev)), vec![VarWrite::Set("--b", "9")]);
    }

    /// `None` 是「移除」，不是「设成空串」——设成空串会触发
    /// *invalid at computed-value time*，取到的是初始值而不是继承来的值
    #[test]
    fn a_none_value_removes_the_variable() {
        let e = entries(&[("--a", None)]);
        let prev = prev_of(&[("--a", Some("1"))]);
        assert_eq!(var_writes(&e, Some(&prev)), vec![VarWrite::Remove("--a")]);
    }

    /// 上一轮就已经是「不存在」，这一轮还是——不必再 remove 一次
    #[test]
    fn an_already_absent_variable_is_left_alone() {
        let e = entries(&[("--a", None)]);
        let prev = prev_of(&[("--a", None)]);
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
        let prev = prev_of(&[("--a", Some("1"))]);
        assert_eq!(
            var_writes(&e, Some(&prev)),
            vec![VarWrite::Set("--b", "2"), VarWrite::Set("--c", "3")]
        );
    }

    #[test]
    fn patch_entries_are_diffed_by_name_and_remove_stale_variables() {
        let previous = entries(&[("--old", Some("1"))]);
        let current = entries(&[("--new", Some("1"))]);
        assert_eq!(
            var_writes(&current, Some(&previous)),
            vec![VarWrite::Set("--new", "1"), VarWrite::Remove("--old")]
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
            theme_entries(&T).expect("matching theme entries"),
            entries(&[("--a", Some("1")), ("--b", Some("2"))])
        );
    }

    /// 名字与取值数量对不上时，所有构建模式都必须报告而不是静默丢弃
    #[test]
    fn a_mismatched_theme_is_reported_in_all_builds() {
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
        assert!(theme_entries(&Broken).is_err());
    }

    #[test]
    fn a_global_theme_value_cannot_open_a_new_rule() {
        struct Malicious;
        impl Display for Malicious {
            fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Ok(())
            }
        }
        impl ThemeToCss for Malicious {
            fn get_variable_values(&self) -> Vec<String> {
                vec![String::from("red; } body { display: none")]
            }

            fn get_variable_names() -> &'static [&'static str] {
                &["--color"]
            }
        }

        let css = global_theme_css(&Malicious).expect("matching theme entries");
        assert!(!css.contains("body { display"), "{css}");
        assert!(!css.contains("; }"), "{css}");
    }
}
