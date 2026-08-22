use crate::element::{Element, TypedElement, tags::Tag};
use crate::view::{
    AnyView, DynamicRenderArgs, DynamicRenderer, MountContext, MountInstance, View, ViewCons,
    mount_dynamic_view_universal,
};
use silex_core::reactivity::{Computed, ReadSignal, Signal, StoredValue};
use silex_core::traits::RxCloneData;
use silex_core::{EffectPhase, Rx, RxRead, SilexError, SilexErrorKind, SilexResult};
use std::borrow::Cow;
use std::fmt::Display;
use web_sys::Node;

pub(crate) fn mount_reactive_text<'scope, T>(
    context: &MountContext<'scope>,
    rx: Rx<'scope, T>,
) -> SilexResult<MountInstance<'scope>>
where
    T: Display + RxCloneData + 'scope,
{
    let owner = context.owner();
    let local_owner = owner.child();
    let node: Node = crate::document().create_text_node("").into();
    context.target().append(&node)?;
    let node_for_cleanup = node.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            if let Some(parent) = node_for_cleanup.parent_node() {
                let _ = parent.remove_child(&node_for_cleanup);
            }
            Ok(())
        }),
        context.error_handler(),
    ) {
        if let Some(parent) = node.parent_node() {
            let _ = parent.remove_child(&node);
        }
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }

    let node_for_effect = node.clone();
    if let Err(error) = local_owner.effect(
        EffectPhase::Normal,
        Box::new(move || -> SilexResult<()> {
            let value = rx.with(|value| value.to_string())?;
            node_for_effect.set_node_value(Some(&value));
            Ok(())
        }),
        context.error_handler(),
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }
    let owner_for_cleanup = local_owner.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            owner_for_cleanup
                .close()
                .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
        }),
        context.error_handler(),
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![node]))
}

pub(crate) fn mount_reactive_view<'scope, V>(
    context: &MountContext<'scope>,
    rx: Rx<'scope, V>,
) -> SilexResult<MountInstance<'scope>>
where
    V: View<'scope> + 'scope,
{
    mount_dynamic_view_universal(
        context,
        DynamicRenderer::new(move |args| {
            let DynamicRenderArgs { context } = args;
            rx.with(|view| view.mount(&context))?
        }),
    )
}

pub trait AutoReactiveView<'scope>: View<'scope> + Sized + 'scope {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        context: &MountContext<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_reactive_view(context, rx)
    }
}

impl<'scope, V> View<'scope> for Rx<'scope, V>
where
    V: AutoReactiveView<'scope>,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        V::mount_reactive(*self, context)
    }
}

macro_rules! impl_auto_reactive_text {
    ($($ty:ty),*) => {
        $(
            impl<'scope> AutoReactiveView<'scope> for $ty {
                fn mount_reactive(
                    rx: Rx<'scope, Self>,
                    context: &MountContext<'scope>,
                ) -> SilexResult<MountInstance<'scope>> {
                    mount_reactive_text(context, rx)
                }
            }
        )*
    };
}

macro_rules! impl_auto_reactive_default {
    ($($ty:ty),*) => {
        $(
            impl<'scope> AutoReactiveView<'scope> for $ty {}
        )*
    };
}

impl_auto_reactive_text!(
    String, bool, char, i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64
);

impl<'scope> AutoReactiveView<'scope> for &'scope str {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        context: &MountContext<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_reactive_text(context, rx)
    }
}

impl<'scope> AutoReactiveView<'scope> for Cow<'scope, str> {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        context: &MountContext<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_reactive_text(context, rx)
    }
}

impl_auto_reactive_default!(Element<'scope>, AnyView<'scope>);

impl<'scope, V> AutoReactiveView<'scope> for Option<V> where V: View<'scope> + 'scope {}

impl<'scope, H, T> AutoReactiveView<'scope> for ViewCons<H, T>
where
    H: View<'scope> + 'scope,
    T: View<'scope> + 'scope,
{
}

impl<'scope, T: Tag + 'scope> AutoReactiveView<'scope> for TypedElement<'scope, T> {}

macro_rules! impl_view_forward_to_rx {
    ($($ty:ident),*) => {
        $(
            impl<'scope, T: 'scope> View<'scope> for $ty<'scope, T>
            where
                T: RxCloneData + 'scope,
                Rx<'scope, T>: View<'scope>,
            {
                fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
                    self.clone()
                        .into_rx()
                        .mount(context)
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, Signal, Computed, StoredValue);
