//! View 与 core reactive source 的桥接。

use crate::flow::dynamic::DynamicRenderer;
use crate::kernel::elements::AnyView;
use crate::kernel::elements::{Element, Tag, TypedElement};
use crate::kernel::{MountContext, MountInstance, View, ViewCons};
use silex_core::reactivity::{Computed, ReadSignal, Signal, StoredValue};
use silex_core::{EffectPhase, Rx, RxReadRef, SilexError, SilexErrorKind, SilexResult};
use std::{borrow::Cow, fmt::Display};

pub(crate) fn mount_reactive_text<'scope, T>(
    context: &MountContext<'scope>,
    rx: Rx<'scope, T>,
) -> SilexResult<MountInstance<'scope>>
where
    T: Display + 'scope,
{
    let owner = context.owner();
    let local_owner = owner.child();
    let node = context.dom().create_text("")?;
    context.target().append_node(&node)?;
    let node_for_cleanup = node.clone();
    let dom_for_cleanup = context.dom().clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            if dom_for_cleanup.parent(&node_for_cleanup)?.is_some() {
                dom_for_cleanup.remove(&node_for_cleanup)?;
            }
            Ok(())
        }),
        context.error_handler(),
    ) {
        if context.dom().parent(&node).ok().flatten().is_some() {
            let _ = context.dom().remove(&node);
        }
        let _ = local_owner.close();
        return Err(error);
    }

    let node_for_effect = node.clone();
    let dom_for_effect = context.dom().clone();
    if let Err(error) = local_owner.effect(
        EffectPhase::Normal,
        Box::new(move || {
            let value = rx.with(|value| value.to_string())?;
            dom_for_effect
                .set_text(&node_for_effect, value)
                .map_err(Into::into)
        }),
        context.error_handler(),
    ) {
        let _ = local_owner.close();
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
        let _ = local_owner.close();
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
    let renderer = DynamicRenderer::new(move |context: MountContext<'scope>| {
        rx.with(|view| context.mount(view))?
    });
    context.mount(&renderer)
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
    ($($ty:ty),* $(,)?) => {
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
    ($($ty:ty),* $(,)?) => {
        $(impl<'scope> AutoReactiveView<'scope> for $ty {})*
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
    ($($ty:ident),* $(,)?) => {
        $(
            impl<'scope, T: 'scope> View<'scope> for $ty<'scope, T>
            where
                Rx<'scope, T>: View<'scope>,
            {
                fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
                    let view = self.clone().into_rx();
                    context.mount(&view)
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, Signal, Computed, StoredValue);
