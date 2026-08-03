use crate::attribute::PendingAttribute;
use crate::element::{Element, TypedElement, tags::Tag};
use crate::view::{
    AnyView, ApplyAttributes, RenderThunk, View, ViewCons, ViewOwner, mount_dynamic_view_universal,
};
use silex_core::error::handle_error;
use silex_core::reactivity::{Memo, ReadSignal, RwSignal, Signal, StoredValue};
use silex_core::traits::RxCloneData;
use silex_core::{Rx, RxValueKind, SilexError};
use std::borrow::Cow;
use std::fmt::Display;
use web_sys::Node;

pub(crate) fn mount_reactive_text<'scope, 'run, T>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    rx: Rx<'scope, 'run, T>,
) where
    T: Display + RxCloneData + 'scope,
{
    let document = crate::document();
    let node = document.create_text_node("");
    if let Err(error) = parent.append_child(&node).map_err(SilexError::from) {
        handle_error(error);
        return;
    }

    owner.effect(Box::new(move || {
        rx.with(|value| node.set_node_value(Some(&value.to_string())));
    }));
}

pub(crate) fn mount_reactive_view<'scope, 'run, V>(
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    rx: Rx<'scope, 'run, V>,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
) where
    V: View<'scope, 'run> + 'scope,
{
    let token = owner.token();
    mount_dynamic_view_universal(
        owner,
        parent,
        attrs,
        RenderThunk::new(move |(parent, attrs)| {
            rx.with(|view| view.mount(&token, &parent, attrs));
        }),
    );
}

pub trait AutoReactiveView<'scope, 'run>: View<'scope, 'run> + Sized + 'scope {
    fn mount_reactive(
        rx: Rx<'scope, 'run, Self>,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_reactive_view(owner, parent, rx, attrs);
    }
}

impl<'scope, 'run, V> ApplyAttributes<'scope, 'run> for Rx<'scope, 'run, V, RxValueKind> where
    V: AutoReactiveView<'scope, 'run>
{
}

impl<'scope, 'run, V> View<'scope, 'run> for Rx<'scope, 'run, V, RxValueKind>
where
    V: AutoReactiveView<'scope, 'run>,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        V::mount_reactive(*self, owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        V::mount_reactive(self, owner, parent, attrs);
    }
}

macro_rules! impl_auto_reactive_text {
    ($($ty:ty),*) => {
        $(
            impl<'scope, 'run> AutoReactiveView<'scope, 'run> for $ty {
                fn mount_reactive(
                    rx: Rx<'scope, 'run, Self>,
                    owner: &dyn ViewOwner<'scope, 'run>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope, 'run>>,
                ) {
                    mount_reactive_text(owner, parent, rx);
                }
            }
        )*
    };
}

macro_rules! impl_auto_reactive_default {
    ($($ty:ty),*) => {
        $(
            impl<'scope, 'run> AutoReactiveView<'scope, 'run> for $ty {}
        )*
    };
}

impl_auto_reactive_text!(
    String, bool, char, i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64
);

impl<'scope, 'run> AutoReactiveView<'scope, 'run> for &'scope str {
    fn mount_reactive(
        rx: Rx<'scope, 'run, Self>,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_reactive_text(owner, parent, rx);
    }
}

impl<'scope, 'run> AutoReactiveView<'scope, 'run> for Cow<'scope, str> {
    fn mount_reactive(
        rx: Rx<'scope, 'run, Self>,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        mount_reactive_text(owner, parent, rx);
    }
}

impl_auto_reactive_default!(Element<'scope, 'run>, AnyView<'scope, 'run>);

impl<'scope, 'run, V> AutoReactiveView<'scope, 'run> for Option<V> where
    V: View<'scope, 'run> + 'scope
{
}

impl<'scope, 'run, H, T> AutoReactiveView<'scope, 'run> for ViewCons<H, T>
where
    H: View<'scope, 'run> + 'scope,
    T: View<'scope, 'run> + 'scope,
{
}

impl<'scope, 'run, T: Tag + 'scope> AutoReactiveView<'scope, 'run>
    for TypedElement<'scope, 'run, T>
{
}

macro_rules! impl_view_forward_to_rx {
    ($($ty:ident),*) => {
        $(
            impl<'scope, 'run, T: 'scope> ApplyAttributes<'scope, 'run> for $ty<'scope, 'run, T>
            where
                T: RxCloneData + 'scope,
                Rx<'scope, 'run, T>: View<'scope, 'run>,
            {
            }

            impl<'scope, 'run, T: 'scope> View<'scope, 'run> for $ty<'scope, 'run, T>
            where
                T: RxCloneData + 'scope,
                Rx<'scope, 'run, T>: View<'scope, 'run>,
            {
                fn mount(
                    &self,
                    owner: &dyn ViewOwner<'scope, 'run>,
                    parent: &Node,
                    attrs: Vec<PendingAttribute<'scope, 'run>>,
                ) {
                    self.clone().into_rx().mount(owner, parent, attrs);
                }

                fn mount_owned(
                    self,
                    owner: &dyn ViewOwner<'scope, 'run>,
                    parent: &Node,
                    attrs: Vec<PendingAttribute<'scope, 'run>>,
                ) where
                    Self: Sized,
                {
                    self.into_rx().mount_owned(owner, parent, attrs);
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, RwSignal, Signal, Memo, StoredValue);
