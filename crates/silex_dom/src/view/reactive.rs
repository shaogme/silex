use crate::attribute::PendingAttribute;
use crate::element::{Element, TypedElement, tags::Tag};
use crate::view::{
    AnyView, ApplyAttributes, OwnedViewOwner, RenderArgs, RenderThunk, View, ViewCons, ViewOwner,
    mount_dynamic_view_universal_from,
};
use silex_core::reactivity::{Memo, ReadSignal, RwSignal, Signal, StoredValue};
use silex_core::traits::RxCloneData;
use silex_core::{Rx, RxValueKind, SilexError, SilexResult};
use std::fmt::Display;
use std::{borrow::Cow, rc::Rc};
use web_sys::Node;

pub(crate) fn mount_reactive_text<'scope, T>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    rx: Rx<'scope, T>,
) -> SilexResult<()>
where
    T: Display + RxCloneData + 'scope,
{
    let inputs = rx.runtime_inputs();
    owner.validate_inputs(&inputs)?;
    let scope = Rc::new(owner.try_owned_scope()?);
    let local_owner = OwnedViewOwner::new(scope.clone(), owner.token().error_handler());
    let parent = parent.clone();
    let node: Node = crate::document().create_text_node("").into();
    parent.append_child(&node)?;
    let node_for_cleanup = node.clone();
    let error_handler = local_owner.token().error_handler();
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
        let _ = scope.dispose();
        return Err(error);
    }

    let node_for_effect = node.clone();
    if let Err(error) = local_owner.effect_from(
        inputs,
        Box::new(move || -> SilexResult<()> {
            let value = rx.try_with(|value| value.to_string())?;
            node_for_effect.set_node_value(Some(&value));
            Ok(())
        }),
        error_handler,
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            let _ = scope_for_cleanup.dispose();
            Ok(())
        }),
        owner.token().error_handler(),
    ) {
        let _ = scope.dispose();
        return Err(error);
    }
    Ok(())
}

pub(crate) fn mount_reactive_view<'scope, V>(
    owner: &dyn ViewOwner<'scope>,
    parent: &Node,
    rx: Rx<'scope, V>,
    attrs: Vec<PendingAttribute<'scope>>,
) -> SilexResult<()>
where
    V: View<'scope> + 'scope,
{
    let inputs = rx.runtime_inputs();
    owner.validate_inputs(&inputs)?;
    mount_dynamic_view_universal_from(
        owner,
        parent,
        attrs,
        inputs,
        RenderThunk::new(move |args| {
            let RenderArgs {
                parent,
                attrs,
                owner: token,
            } = args;
            rx.try_with(|view| view.mount(&token, &parent, attrs))
                .map_err(SilexError::from)
                .and_then(|result| result)
        }),
    )
}

pub trait AutoReactiveView<'scope>: View<'scope> + Sized + 'scope {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        mount_reactive_view(owner, parent, rx, attrs)
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
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        V::mount_reactive(*self, owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        V::mount_reactive(self, owner, parent, attrs)
    }
}

macro_rules! impl_auto_reactive_text {
    ($($ty:ty),*) => {
        $(
            impl<'scope> AutoReactiveView<'scope> for $ty {
                fn mount_reactive(
                    rx: Rx<'scope, Self>,
                    owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
                ) -> SilexResult<()> {
                    mount_reactive_text(owner, parent, rx)
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
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        mount_reactive_text(owner, parent, rx)
    }
}

impl<'scope> AutoReactiveView<'scope> for Cow<'scope, str> {
    fn mount_reactive(
        rx: Rx<'scope, Self>,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        mount_reactive_text(owner, parent, rx)
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
                    owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    attrs: Vec<PendingAttribute<'scope>>,
                ) -> SilexResult<()> {
                    self.clone().into_rx().mount(owner, parent, attrs)
                }

                fn mount_owned(
                    self,
                    owner: &dyn ViewOwner<'scope>,
                    parent: &Node,
                    attrs: Vec<PendingAttribute<'scope>>,
                ) -> SilexResult<()> where
                    Self: Sized,
                {
                    self.into_rx().mount_owned(owner, parent, attrs)
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, RwSignal, Signal, Memo, StoredValue);
