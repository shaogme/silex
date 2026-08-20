use super::contract::{ApplyAttributes, MountInstance, View, ViewCons, ViewNil};
use super::owner::{MountErrorHandler, MountOwner, MountOwnerToken, OwnerMount};
use crate::attribute::AttrOp;
use silex_core::{CloseError, SilexError, SilexErrorKind, SilexResult};
use std::{
    borrow::Cow,
    panic::{AssertUnwindSafe, catch_unwind},
};
use web_sys::Node;

pub(crate) fn mount_composite<'scope, F>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<AttrOp<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    mount: F,
) -> SilexResult<MountInstance<'scope>>
where
    F: FnOnce(
        &dyn MountOwner<'scope>,
        &Node,
        Vec<AttrOp<'scope>>,
        MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>>,
{
    let provisional_owner = OwnerMount::new(owner.child());
    let fragment: Node = crate::document().create_document_fragment().into();

    if let Err(error) = mount(&provisional_owner, &fragment, attrs, error_handler) {
        return rollback_composite_scope_with_primary(owner, &provisional_owner, error);
    }

    let fragment_children = fragment.child_nodes();
    let nodes = (0..fragment_children.length())
        .filter_map(|index| fragment_children.item(index))
        .collect();

    let owner_for_cleanup = provisional_owner.token();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            owner_for_cleanup
                .close()
                .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
        }),
        error_handler,
    ) {
        return rollback_composite_scope_with_primary(owner, &provisional_owner, error);
    }

    if let Err(error) = parent.append_child(&fragment).map_err(SilexError::fatal) {
        return rollback_composite_scope_with_primary(owner, &provisional_owner, error);
    }
    Ok(MountInstance::from_nodes(nodes))
}

#[doc(hidden)]
pub fn mount_component<'scope, F>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<AttrOp<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    mount: F,
) -> SilexResult<MountInstance<'scope>>
where
    F: FnOnce(
        &dyn MountOwner<'scope>,
        &Node,
        Vec<AttrOp<'scope>>,
        MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>>,
{
    mount_composite(owner, parent, attrs, error_handler, mount)
}

fn rollback_composite_scope<'scope>(owner: &MountOwnerToken<'scope>) -> Result<(), CloseError> {
    match catch_unwind(AssertUnwindSafe(|| owner.close())) {
        Ok(result) => result,
        Err(panic) => Err(CloseError::from_panic(panic)),
    }
}

fn rollback_composite_scope_with_primary<'scope>(
    owner: &dyn MountOwner<'scope>,
    provisional_owner: &OwnerMount<'scope>,
    primary: SilexError,
) -> SilexResult<MountInstance<'scope>> {
    match rollback_composite_scope(&provisional_owner.token()) {
        Ok(()) => Err(primary),
        Err(cleanup) => {
            owner.token().report_close_error(cleanup);
            Err(primary.into_fatal())
        }
    }
}

pub fn mount_text_node<'scope>(parent: &Node, text: &str) -> SilexResult<MountInstance<'scope>> {
    let document = crate::document();
    let node = document.create_text_node(text);
    parent.append_child(&node).map_err(SilexError::fatal)?;
    Ok(MountInstance::from_nodes(vec![node.into()]))
}

macro_rules! impl_text_view {
    ($ty:ty) => {
        impl<'scope> ApplyAttributes<'scope> for $ty {}

        impl<'scope> View<'scope> for $ty {
            fn mount(
                &self,
                owner: &dyn MountOwner<'scope>,
                parent: &Node,
                _attrs: Vec<AttrOp<'scope>>,
                _error_handler: MountErrorHandler<'scope>,
            ) -> SilexResult<MountInstance<'scope>> {
                let _ = owner;
                mount_text_node(parent, self)
            }
        }
    };
}

impl_text_view!(String);

impl<'scope> ApplyAttributes<'scope> for &'scope str {}

impl<'scope> View<'scope> for &'scope str {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<AttrOp<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let _ = owner;
        mount_text_node(parent, self)
    }
}

impl<'scope> ApplyAttributes<'scope> for Cow<'scope, str> {}

impl<'scope> View<'scope> for Cow<'scope, str> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<AttrOp<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let _ = owner;
        mount_text_node(parent, self.as_ref())
    }
}

macro_rules! impl_primitive_view {
    ($($ty:ty),*) => {
        $(
            impl<'scope> ApplyAttributes<'scope> for $ty {}

            impl<'scope> View<'scope> for $ty {
                fn mount(
                    &self,
                    owner: &dyn MountOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<AttrOp<'scope>>,
                    _error_handler: MountErrorHandler<'scope>,
                ) -> SilexResult<MountInstance<'scope>> {
                    let _ = owner;
                    mount_text_node(parent, &self.to_string())
                }
            }
        )*
    };
}

impl_primitive_view!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64, bool, char
);

impl<'scope> ApplyAttributes<'scope> for () {}

impl<'scope> View<'scope> for () {
    fn mount(
        &self,
        _owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<AttrOp<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        Ok(MountInstance::from_nodes(Vec::new()))
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Option<V> {
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
        if let Some(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Option<V> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        if let Some(value) = self {
            value.mount(owner, parent, attrs, error_handler)
        } else {
            Ok(MountInstance::from_nodes(Vec::new()))
        }
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Vec<V> {
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Vec<V> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.iter().enumerate() {
                    let _ = value.mount(
                        transaction_owner,
                        fragment,
                        if index == 0 {
                            attrs.clone()
                        } else {
                            Vec::new()
                        },
                        error_handler,
                    )?;
                }
                Ok(MountInstance::from_nodes(Vec::new()))
            },
        )
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>, const N: usize> ApplyAttributes<'scope>
    for [V; N]
{
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
        for value in self {
            value.apply_attributes(attrs.clone());
        }
    }
}

impl<'scope, V: View<'scope>, const N: usize> View<'scope> for [V; N] {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.iter().enumerate() {
                    let _ = value.mount(
                        transaction_owner,
                        fragment,
                        if index == 0 {
                            attrs.clone()
                        } else {
                            Vec::new()
                        },
                        error_handler,
                    )?;
                }
                Ok(MountInstance::from_nodes(Vec::new()))
            },
        )
    }
}

impl<'scope> ApplyAttributes<'scope> for ViewNil {}

impl<'scope> View<'scope> for ViewNil {
    fn mount(
        &self,
        _owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<AttrOp<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        Ok(MountInstance::from_nodes(Vec::new()))
    }
}

impl<'scope, H: ApplyAttributes<'scope>, T: ApplyAttributes<'scope>> ApplyAttributes<'scope>
    for ViewCons<H, T>
{
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

impl<'scope, H: View<'scope>, T: View<'scope>> View<'scope> for ViewCons<H, T> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                let _ = self
                    .0
                    .mount(transaction_owner, fragment, attrs, error_handler)?;
                let _ = self
                    .1
                    .mount(transaction_owner, fragment, Vec::new(), error_handler)?;
                Ok(MountInstance::from_nodes(Vec::new()))
            },
        )
    }
}

#[macro_export]
macro_rules! chain {
    () => {
        $crate::view::ViewNil
    };
    ($head:expr $(,)?) => {
        $crate::view::ViewCons($head, $crate::view::ViewNil)
    };
    ($head:expr, $($tail:expr),+ $(,)?) => {
        $crate::view::ViewCons($head, $crate::chain!($($tail),+))
    };
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for SilexResult<V> {
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
        if let Ok(value) = self {
            value.apply_attributes(attrs);
        }
    }
}

impl<'scope, V: View<'scope>> View<'scope> for SilexResult<V> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        match self {
            Ok(value) => value.mount(owner, parent, attrs, error_handler),
            Err(error) => Err(error.clone()),
        }
    }
}
