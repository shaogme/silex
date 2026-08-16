use crate::attribute::PendingAttribute;
use crate::element::{Element, TypedElement, tags::Tag};
use crate::view::{
    AnyView, ApplyAttributes, DynamicRenderArgs, DynamicRenderer, MountErrorHandler, MountInstance,
    MountOwner, View, ViewCons, mount_dynamic_view_universal,
};
use silex_core::reactivity::{Computed, ReadSignal, RwSignal, Signal, StoredValue};
use silex_core::traits::RxCloneData;
use silex_core::{Rx, RxValueKind, SilexError, SilexErrorKind, SilexResult};
use std::borrow::Cow;
use std::fmt::Display;
use web_sys::Node;

pub(crate) fn mount_reactive_text<'scope, T>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    rx: Rx<'scope, T>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<MountInstance<'scope>>
where
    T: Display + RxCloneData + 'scope,
{
    let local_owner = owner.child();
    let parent = parent.clone();
    let node: Node = crate::document().create_text_node("").into();
    parent.append_child(&node).map_err(SilexError::fatal)?;
    let node_for_cleanup = node.clone();
    if let Err(error) = local_owner.on_cleanup(
        Box::new(move || {
            if let Some(parent) = node_for_cleanup.parent_node() {
                let _ = parent.remove_child(&node_for_cleanup);
            }
            Ok(())
        }),
        error_handler,
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
        Box::new(move || -> SilexResult<()> {
            let value = rx.with(|value| value.to_string())?;
            node_for_effect.set_node_value(Some(&value));
            Ok(())
        }),
        error_handler,
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
        error_handler,
    ) {
        if let Err(close_error) = local_owner.close() {
            local_owner.report_close_error(close_error);
        }
        return Err(error);
    }
    Ok(MountInstance::from_nodes(vec![node]))
}

pub(crate) fn mount_reactive_view<'scope, V>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    rx: Rx<'scope, V>,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: MountErrorHandler<'scope>,
) -> SilexResult<MountInstance<'scope>>
where
    V: View<'scope> + 'scope,
{
    mount_dynamic_view_universal(
        owner,
        parent,
        attrs,
        error_handler,
        DynamicRenderer::new(move |args| {
            let DynamicRenderArgs {
                parent,
                attrs,
                owner: token,
                error_handler,
            } = args;
            rx.with(|view| view.mount(&token, &parent, attrs, error_handler))?
        }),
    )
}

pub trait AutoReactiveView<'scope>: View<'scope> + Sized + 'scope {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_reactive_view(owner, parent, rx, attrs, error_handler)
    }
}

impl<'scope, V> ApplyAttributes<'scope> for Rx<'scope, V, RxValueKind> where
    V: AutoReactiveView<'scope>
{
}

impl<'scope, V> View<'scope> for Rx<'scope, V, RxValueKind>
where
    V: AutoReactiveView<'scope>,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        V::mount_reactive(*self, owner, parent, attrs, error_handler)
    }
}

macro_rules! impl_auto_reactive_text {
    ($($ty:ty),*) => {
        $(
            impl<'scope> AutoReactiveView<'scope> for $ty {
                fn mount_reactive(
                    rx: Rx<'scope, Self>,
                    owner: &dyn MountOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
                    error_handler: MountErrorHandler<'scope>,
                ) -> SilexResult<MountInstance<'scope>> {
                    mount_reactive_text(owner, parent, rx, error_handler)
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
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_reactive_text(owner, parent, rx, error_handler)
    }
}

impl<'scope> AutoReactiveView<'scope> for Cow<'scope, str> {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_reactive_text(owner, parent, rx, error_handler)
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
            impl<'scope, T: 'scope> ApplyAttributes<'scope> for $ty<'scope, T>
            where
                T: RxCloneData + 'scope,
                Rx<'scope, T>: View<'scope>,
            {
            }

            impl<'scope, T: 'scope> View<'scope> for $ty<'scope, T>
            where
                T: RxCloneData + 'scope,
                Rx<'scope, T>: View<'scope>,
            {
                fn mount(
                    &self,
                    owner: &dyn MountOwner<'scope>,
                    parent: &Node,
                    attrs: Vec<PendingAttribute<'scope>>,
                    error_handler: MountErrorHandler<'scope>,
                ) -> SilexResult<MountInstance<'scope>> {
                    self.clone()
                        .into_rx()
                        .mount(owner, parent, attrs, error_handler)
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, RwSignal, Signal, Computed, StoredValue);
