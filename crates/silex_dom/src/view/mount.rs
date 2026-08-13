use super::contract::{ApplyAttributes, View, ViewCons, ViewNil};
use super::owner::{MountErrorHandler, MountOwner, OwnedMountOwner};
use crate::attribute::PendingAttribute;
use silex_core::{CleanupError, OwnedScope, SilexError, SilexResult};
use std::{
    borrow::Cow,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use web_sys::Node;

pub(crate) fn mount_composite<'scope, F>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    mount: F,
) -> SilexResult<()>
where
    F: FnOnce(
        &dyn MountOwner<'scope>,
        &Node,
        Vec<PendingAttribute<'scope>>,
        MountErrorHandler<'scope>,
    ) -> SilexResult<()>,
{
    let scope = Rc::new(owner.owned_scope()?);
    let owner_token = owner.token();
    let provisional_owner = owner_token.cleanup_reporter().map_or_else(
        || OwnedMountOwner::new(scope.clone()),
        |reporter| OwnedMountOwner::with_cleanup_reporter(scope.clone(), reporter),
    );
    let fragment: Node = crate::document().create_document_fragment().into();

    if let Err(error) = mount(&provisional_owner, &fragment, attrs, error_handler) {
        return rollback_composite_scope_with_primary(owner, &scope, error);
    }

    let scope_for_cleanup = scope.clone();
    if let Err(error) = owner.on_cleanup(
        Box::new(move || {
            let _ = scope_for_cleanup.dispose();
            Ok(())
        }),
        error_handler,
    ) {
        return rollback_composite_scope_with_primary(owner, &scope, error);
    }

    if let Err(error) = parent.append_child(&fragment).map_err(SilexError::fatal) {
        return rollback_composite_scope_with_primary(owner, &scope, error);
    }
    Ok(())
}

#[doc(hidden)]
pub fn mount_component<'scope, F>(
    owner: &dyn MountOwner<'scope>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope>>,
    error_handler: MountErrorHandler<'scope>,
    mount: F,
) -> SilexResult<()>
where
    F: FnOnce(
        &dyn MountOwner<'scope>,
        &Node,
        Vec<PendingAttribute<'scope>>,
        MountErrorHandler<'scope>,
    ) -> SilexResult<()>,
{
    mount_composite(owner, parent, attrs, error_handler, mount)
}

fn rollback_composite_scope<'scope>(scope: &Rc<OwnedScope<'scope>>) -> Result<(), CleanupError> {
    match catch_unwind(AssertUnwindSafe(|| scope.dispose())) {
        Ok(result) => result,
        Err(panic) => resume_unwind(panic),
    }
}

fn rollback_composite_scope_with_primary<'scope>(
    owner: &dyn MountOwner<'scope>,
    scope: &Rc<OwnedScope<'scope>>,
    primary: SilexError,
) -> SilexResult<()> {
    match rollback_composite_scope(scope) {
        Ok(()) => Err(primary),
        Err(cleanup) => {
            if let Some(reporter) = owner.token().cleanup_reporter() {
                reporter(cleanup);
            } else {
                let _ = cleanup.into_diagnostic();
            }
            Err(primary.into_fatal())
        }
    }
}

pub fn mount_text_node(parent: &Node, text: &str) -> SilexResult<()> {
    let document = crate::document();
    let node = document.create_text_node(text);
    parent.append_child(&node).map_err(SilexError::fatal)?;
    Ok(())
}

macro_rules! impl_text_view {
    ($ty:ty) => {
        impl<'scope> ApplyAttributes<'scope> for $ty {}

        impl<'scope> View<'scope> for $ty {
            fn mount(
                &self,
                owner: &dyn MountOwner<'scope>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope>>,
                _error_handler: MountErrorHandler<'scope>,
            ) -> SilexResult<()> {
                let _ = owner;
                mount_text_node(parent, self)
            }

            fn mount_owned(
                self,
                owner: &dyn MountOwner<'scope>,
                parent: &Node,
                _attrs: Vec<PendingAttribute<'scope>>,
                _error_handler: MountErrorHandler<'scope>,
            ) -> SilexResult<()>
            where
                Self: Sized,
            {
                let _ = owner;
                mount_text_node(parent, &self)
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
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let _ = owner;
        mount_text_node(parent, self)
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
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
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        let _ = owner;
        mount_text_node(parent, self.as_ref())
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
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
                    _attrs: Vec<PendingAttribute<'scope>>,
                    _error_handler: MountErrorHandler<'scope>,
                ) -> SilexResult<()> {
                    let _ = owner;
                    mount_text_node(parent, &self.to_string())
                }

                fn mount_owned(
                    self,
                    owner: &dyn MountOwner<'scope>,
                    parent: &Node,
                    _attrs: Vec<PendingAttribute<'scope>>,
                    _error_handler: MountErrorHandler<'scope>,
                ) -> SilexResult<()> where
                    Self: Sized,
                {
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
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        Ok(())
    }

    fn mount_owned(
        self,
        _owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Option<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
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
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        if let Some(value) = self {
            value.mount(owner, parent, attrs, error_handler)
        } else {
            Ok(())
        }
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        if let Some(value) = self {
            value.mount_owned(owner, parent, attrs, error_handler)
        } else {
            Ok(())
        }
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>> ApplyAttributes<'scope> for Vec<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
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
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.iter().enumerate() {
                    value.mount(
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
                Ok(())
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.into_iter().enumerate() {
                    value.mount_owned(
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
                Ok(())
            },
        )
    }
}

impl<'scope, V: View<'scope> + ApplyAttributes<'scope>, const N: usize> ApplyAttributes<'scope>
    for [V; N]
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
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
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.iter().enumerate() {
                    value.mount(
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
                Ok(())
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                for (index, value) in self.into_iter().enumerate() {
                    value.mount_owned(
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
                Ok(())
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
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        Ok(())
    }

    fn mount_owned(
        self,
        _owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        Ok(())
    }
}

impl<'scope, H: ApplyAttributes<'scope>, T: ApplyAttributes<'scope>> ApplyAttributes<'scope>
    for ViewCons<H, T>
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

impl<'scope, H: View<'scope>, T: View<'scope>> View<'scope> for ViewCons<H, T> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                self.0
                    .mount(transaction_owner, fragment, attrs, error_handler)?;
                self.1
                    .mount(transaction_owner, fragment, Vec::new(), error_handler)
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let ViewCons(head, tail) = self;
        mount_composite(
            owner,
            parent,
            attrs,
            error_handler,
            move |transaction_owner, fragment, attrs, error_handler| {
                head.mount_owned(transaction_owner, fragment, attrs, error_handler)?;
                tail.mount_owned(transaction_owner, fragment, Vec::new(), error_handler)
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
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
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
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        match self {
            Ok(value) => value.mount(owner, parent, attrs, error_handler),
            Err(error) => Err(error.clone()),
        }
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        match self {
            Ok(value) => value.mount_owned(owner, parent, attrs, error_handler),
            Err(error) => Err(error),
        }
    }
}
