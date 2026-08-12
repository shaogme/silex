use std::borrow::Cow;

use silex_core::{Rx, RxValueKind, SilexError, SilexResult};
use web_sys::Element as WebElem;

use super::foundation::{ApplyTarget, ApplyToDom, ReactiveApply, apply_immediate_string};
use crate::attribute::op::{
    Attr, AttrData, AttrOp, AttrUpdate, apply_attr_with_target_internal,
    apply_immediate_bool_internal, get_style_decl,
};
use crate::view::ViewOwnerToken;

fn register<'scope>(
    owner: &ViewOwnerToken<'scope>,
    inputs: silex_core::RuntimeInputs,
    callback: impl FnMut() -> SilexResult<()> + 'scope,
) -> SilexResult<()> {
    owner.effect_from(inputs, Box::new(callback), owner.error_handler())
}

pub(crate) fn apply_primitive_reactive_internal<'scope, T>(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<'scope, T>,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()>
where
    T: ToString + Clone + 'scope,
{
    register(owner, rx.runtime_inputs(), move || {
        let value = rx.try_get()?.to_string();
        match &target {
            ApplyTarget::Attr(_) => apply_immediate_string(&el, &target, &value),
            ApplyTarget::Prop(_) => apply_immediate_string(&el, &target, &value),
            ApplyTarget::Known(prop) => apply_attr_with_target_internal(
                &el,
                prop.name(),
                ApplyTarget::Known(*prop),
                &Attr::from(value),
            ),
            ApplyTarget::Class => el.set_attribute("class", &value).map_err(SilexError::from),
            ApplyTarget::Style => {
                if let Some(style) = get_style_decl(&el) {
                    style.set_css_text(&value);
                    Ok(())
                } else {
                    Err(SilexError::Dom(
                        "element does not expose a style declaration".into(),
                    ))
                }
            }
            ApplyTarget::Apply => Ok(()),
        }
    })
}

pub(crate) fn apply_string_reactive_internal<'scope>(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<'scope, String>,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()> {
    apply_primitive_reactive_internal(el, target, rx, owner)
}

pub(crate) fn apply_string_pair_reactive_internal<'scope>(
    el: WebElem,
    key: Cow<'static, str>,
    target: ApplyTarget,
    rx: Rx<'scope, String>,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()> {
    if matches!(target, ApplyTarget::Style) {
        let style = get_style_decl(&el)
            .ok_or_else(|| SilexError::Dom("element does not expose a style declaration".into()))?;
        register(owner, rx.runtime_inputs(), move || {
            let value = rx.try_get()?;
            style.set_property(&key, &value).map_err(SilexError::from)
        })?;
    } else {
        apply_string_reactive_internal(el, target, rx, owner)?;
    }
    Ok(())
}

pub(crate) fn apply_bool_reactive_internal<'scope>(
    el: WebElem,
    target: ApplyTarget,
    rx: Rx<'scope, bool>,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()> {
    register(owner, rx.runtime_inputs(), move || {
        let value = rx.try_get()?;
        match &target {
            ApplyTarget::Attr(name) => {
                if value {
                    el.set_attribute(name, "").map_err(SilexError::from)
                } else {
                    el.remove_attribute(name).map_err(SilexError::from)
                }
            }
            ApplyTarget::Prop(name) => apply_immediate_bool_internal(&el, name, value, true),
            ApplyTarget::Known(prop) => apply_attr_with_target_internal(
                &el,
                prop.name(),
                ApplyTarget::Known(*prop),
                &Attr::from(value),
            ),
            ApplyTarget::Class => el
                .class_list()
                .toggle_with_force("active", value)
                .map(|_| ())
                .map_err(SilexError::from),
            _ => Ok(()),
        }
    })
}

pub(crate) fn apply_bool_pair_reactive_internal<'scope>(
    el: WebElem,
    key: Cow<'static, str>,
    rx: Rx<'scope, bool>,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()> {
    let list = el.class_list();
    register(owner, rx.runtime_inputs(), move || {
        if rx.try_get()? {
            list.add_1(&key).map_err(SilexError::from)
        } else {
            list.remove_1(&key).map_err(SilexError::from)
        }
    })
}

pub(crate) fn apply_rx_internal<'scope, T>(
    rx: Rx<'scope, T>,
    el: &WebElem,
    target: ApplyTarget,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()>
where
    T: ReactiveApply<'scope> + 'scope,
{
    T::apply_to_dom(rx, el.clone(), target, owner)
}

impl<'scope, T> ApplyToDom<'scope> for Rx<'scope, T, RxValueKind>
where
    T: ReactiveApply<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        el: &WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        apply_rx_internal(*self, el, target, owner)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        if let Some(op) = T::into_op_reactive(self, target.clone()) {
            op
        } else {
            AttrOp::custom_with_inputs(self.runtime_inputs(), move |el, owner| {
                apply_rx_internal(self, el, target.clone(), owner)
            })
        }
    }
}

impl<'scope> ReactiveApply<'scope> for String {
    fn apply_to_dom(
        rx: Rx<'scope, Self>,
        el: WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        apply_string_reactive_internal(el, target, rx, owner)
    }

    fn apply_pair(
        rx: Rx<'scope, Self>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        apply_string_pair_reactive_internal(el, key, target, rx, owner)
    }

    fn into_op_reactive(rx: Rx<'scope, Self>, target: ApplyTarget) -> Option<AttrOp<'scope>> {
        match target {
            ApplyTarget::Attr(_) | ApplyTarget::Known(_) => Some(AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::ReactiveString(rx),
            })),
            ApplyTarget::Class => Some(AttrOp::reactive_classes(rx)),
            ApplyTarget::Style => Some(AttrOp::reactive_stylesheet(rx)),
            _ => None,
        }
    }

    fn into_op_pair_reactive(
        rx: Rx<'scope, Self>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp<'scope>> {
        matches!(target, ApplyTarget::Style).then(|| AttrOp::style_property(key, rx))
    }
}

macro_rules! impl_reactive_apply_primitive {
    ($($ty:ty),*) => {
        $(
            impl<'scope> ReactiveApply<'scope> for $ty {
                fn apply_to_dom(
                    rx: Rx<'scope, Self>,
                    el: WebElem,
                    target: ApplyTarget,
                    owner: &ViewOwnerToken<'scope>,
                ) -> SilexResult<()> {
                    apply_primitive_reactive_internal(el, target, rx, owner)
                }

                fn apply_pair(
                    rx: Rx<'scope, Self>,
                    key: Cow<'static, str>,
                    el: WebElem,
                    target: ApplyTarget,
                    owner: &ViewOwnerToken<'scope>,
                ) -> SilexResult<()> {
                    let _ = (rx, key, el, target, owner);
                    Ok(())
                }

                fn into_op_reactive(
                    rx: Rx<'scope, Self>,
                    target: ApplyTarget,
                ) -> Option<AttrOp<'scope>> {
                    let _ = rx;
                    let _ = target;
                    None
                }

                fn into_op_pair_reactive(
                    rx: Rx<'scope, Self>,
                    key: Cow<'static, str>,
                    target: ApplyTarget,
                ) -> Option<AttrOp<'scope>> {
                    let _ = (rx, key, target);
                    None
                }
            }
        )*
    };
}

impl_reactive_apply_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl<'scope> ReactiveApply<'scope> for bool {
    fn apply_to_dom(
        rx: Rx<'scope, Self>,
        el: WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        apply_bool_reactive_internal(el, target, rx, owner)
    }

    fn apply_pair(
        rx: Rx<'scope, Self>,
        key: Cow<'static, str>,
        el: WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        if matches!(target, ApplyTarget::Class) {
            apply_bool_pair_reactive_internal(el, key, rx, owner)
        } else {
            Ok(())
        }
    }

    fn into_op_reactive(rx: Rx<'scope, Self>, target: ApplyTarget) -> Option<AttrOp<'scope>> {
        matches!(
            target,
            ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_)
        )
        .then(|| {
            AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::ReactiveBool(rx),
            })
        })
    }

    fn into_op_pair_reactive(
        rx: Rx<'scope, Self>,
        key: Cow<'static, str>,
        target: ApplyTarget,
    ) -> Option<AttrOp<'scope>> {
        matches!(target, ApplyTarget::Class).then(|| AttrOp::class_toggle(key, rx))
    }
}

impl<'scope> ReactiveApply<'scope> for Attr<'scope> {
    fn apply_to_dom(
        rx: Rx<'scope, Self>,
        el: WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        register(owner, rx.runtime_inputs(), move || {
            if let Some(name) = target.name() {
                let value = rx.try_get()?;
                apply_attr_with_target_internal(&el, &name, target.clone(), &value)
            } else {
                Ok(())
            }
        })
    }

    fn into_op_reactive(rx: Rx<'scope, Self>, target: ApplyTarget) -> Option<AttrOp<'scope>> {
        matches!(
            target,
            ApplyTarget::Attr(_) | ApplyTarget::Prop(_) | ApplyTarget::Known(_)
        )
        .then(|| {
            AttrOp::Update(AttrUpdate {
                target,
                data: AttrData::ReactiveAttr(rx),
            })
        })
    }
}

impl<'scope, T> ReactiveApply<'scope> for Option<T>
where
    T: ToString + Clone + 'scope,
{
    fn apply_to_dom(
        rx: Rx<'scope, Self>,
        el: WebElem,
        target: ApplyTarget,
        owner: &ViewOwnerToken<'scope>,
    ) -> SilexResult<()> {
        register(owner, rx.runtime_inputs(), move || {
            let value = rx
                .try_get()?
                .map(|value| value.to_string())
                .unwrap_or_default();
            match target {
                ApplyTarget::Class => el.set_attribute("class", &value).map_err(SilexError::from),
                ApplyTarget::Style => {
                    if let Some(style) = get_style_decl(&el) {
                        style.set_css_text(&value);
                        Ok(())
                    } else {
                        Err(SilexError::Dom(
                            "element does not expose a style declaration".into(),
                        ))
                    }
                }
                _ => apply_immediate_string(&el, &target, &value),
            }
        })
    }
}

fn map_string_like_rx<'scope, T>(rx: Rx<'scope, T>) -> Rx<'scope, String>
where
    T: ToString + 'scope,
{
    rx.map(
        |value| value.to_string(),
        rx.scope()
            .error_handler(|error| panic!("reactive attribute mapping failed: {error}")),
    )
    .unwrap_or_else(|error| panic!("创建 reactive attribute mapping 失败: {error}"))
}

macro_rules! impl_reactive_apply_string_like {
    ($($ty:ty),*) => {
        $(
            impl<'scope, 'a: 'scope> ReactiveApply<'scope> for $ty {
                fn apply_to_dom(
                    rx: Rx<'scope, Self>,
                    el: WebElem,
                    target: ApplyTarget,
                    owner: &ViewOwnerToken<'scope>,
                ) -> SilexResult<()> {
                    apply_primitive_reactive_internal(el, target, rx, owner)
                }

                fn apply_pair(
                    rx: Rx<'scope, Self>,
                    key: Cow<'static, str>,
                    el: WebElem,
                    target: ApplyTarget,
                    owner: &ViewOwnerToken<'scope>,
                ) -> SilexResult<()> {
                    if matches!(target, ApplyTarget::Style) {
                        let style = get_style_decl(&el).ok_or_else(|| {
                            SilexError::Dom("element does not expose a style declaration".into())
                        })?;
                        register(owner, rx.runtime_inputs(), move || {
                            let value = rx.try_get()?;
                            style
                                .set_property(&key, value.as_ref())
                                .map_err(SilexError::from)
                        })?;
                        Ok(())
                    } else {
                        apply_primitive_reactive_internal(el, target, rx, owner)
                    }
                }

                fn into_op_reactive(
                    rx: Rx<'scope, Self>,
                    target: ApplyTarget,
                ) -> Option<AttrOp<'scope>> {
                    match target {
                        ApplyTarget::Attr(_) | ApplyTarget::Known(_) => {
                            Some(AttrOp::Update(AttrUpdate {
                                target,
                                data: AttrData::ReactiveString(map_string_like_rx(rx)),
                            }))
                        }
                        ApplyTarget::Class => {
                            Some(AttrOp::reactive_classes(map_string_like_rx(rx)))
                        }
                        ApplyTarget::Style => {
                            Some(AttrOp::reactive_stylesheet(map_string_like_rx(rx)))
                        }
                        _ => None,
                    }
                }

                fn into_op_pair_reactive(
                    rx: Rx<'scope, Self>,
                    key: Cow<'static, str>,
                    target: ApplyTarget,
                ) -> Option<AttrOp<'scope>> {
                    if matches!(target, ApplyTarget::Style) {
                        Some(AttrOp::style_property(key, map_string_like_rx(rx)))
                    } else {
                        None
                    }
                }
            }
        )*
    };
}

impl_reactive_apply_string_like!(&'a str, Cow<'a, str>, &'a String);
