use crate::context::{MountContext, MountTarget, MountTransaction};
use crate::contract::{MountInstance, View, ViewCons, ViewNil};
use crate::owner::{MountOwner, OwnerMount};
use silex_core::{CloseError, SilexError, SilexErrorKind, SilexResult};
use silex_dom::model::DomNode;
use std::{
    borrow::Cow,
    panic::{AssertUnwindSafe, catch_unwind},
};

impl<'scope> MountContext<'scope> {
    pub(crate) fn mount_composite<F>(&self, mount: F) -> SilexResult<MountInstance<'scope>>
    where
        F: FnOnce(&MountContext<'scope>) -> SilexResult<MountInstance<'scope>>,
    {
        let owner = self.owner();
        let transaction = self.transaction().child()?;
        let provisional_owner = OwnerMount::new(owner.child());
        let fragment = self.dom().create_fragment()?;
        let child_context = self.with_parts(
            MountTarget::append(self.dom().clone(), fragment.clone()),
            self.ancestry().clone(),
            provisional_owner.token(),
            transaction.clone(),
        );

        if let Err(error) = mount(&child_context) {
            return self.rollback_composite(
                &transaction,
                &provisional_owner,
                &fragment,
                &[],
                error,
            );
        }
        let nodes = match self.dom().children(&fragment) {
            Ok(nodes) => nodes,
            Err(error) => {
                return self.rollback_composite(
                    &transaction,
                    &provisional_owner,
                    &fragment,
                    &[],
                    error.into(),
                );
            }
        };
        let owner_for_cleanup = provisional_owner.token();
        if let Err(error) = owner.on_cleanup(
            Box::new(move || {
                owner_for_cleanup
                    .close()
                    .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
            }),
            self.error_handler(),
        ) {
            return self.rollback_composite(
                &transaction,
                &provisional_owner,
                &fragment,
                &nodes,
                error,
            );
        }
        if let Err(error) = self.target().append_node(&fragment) {
            return self.rollback_composite(
                &transaction,
                &provisional_owner,
                &fragment,
                &nodes,
                error,
            );
        }
        if let Err(error) = transaction.commit() {
            return self.rollback_composite(
                &transaction,
                &provisional_owner,
                &fragment,
                &nodes,
                error,
            );
        }
        Ok(MountInstance::from_nodes(nodes))
    }

    fn rollback_composite(
        &self,
        transaction: &MountTransaction<'scope>,
        owner: &OwnerMount<'scope>,
        fragment: &DomNode,
        nodes: &[DomNode],
        primary: SilexError,
    ) -> SilexResult<MountInstance<'scope>> {
        let _ = transaction.rollback();
        if let Ok(nodes) = self.dom().children(fragment) {
            for node in nodes {
                let _ = self.dom().remove(&node);
            }
        }
        let remove_nodes = || {
            for node in nodes {
                if self.dom().parent(node).ok().flatten().is_some() {
                    let _ = self.dom().remove(node);
                }
            }
        };
        match catch_unwind(AssertUnwindSafe(|| owner.token().close())) {
            Ok(Ok(())) => {
                remove_nodes();
                Err(primary)
            }
            Ok(Err(error)) => {
                owner.token().report_close_error(error);
                remove_nodes();
                Err(primary.into_fatal())
            }
            Err(panic) => {
                owner
                    .token()
                    .report_close_error(CloseError::from_panic(panic));
                remove_nodes();
                Err(primary)
            }
        }
    }

    fn mount_text(&self, text: &str) -> SilexResult<MountInstance<'scope>> {
        let cleanup_dom = self.dom().clone();
        let node = self.dom().create_text(text)?;
        self.target().append_node(&node)?;
        let cleanup_node = node.clone();
        if let Err(error) = self.owner().on_cleanup(
            Box::new(move || {
                if cleanup_dom.parent(&cleanup_node)?.is_some() {
                    cleanup_dom.remove(&cleanup_node)?;
                }
                Ok(())
            }),
            self.error_handler(),
        ) {
            let _ = self.dom().remove(&node);
            return Err(error);
        }
        Ok(MountInstance::from_nodes(vec![node]))
    }
}

macro_rules! impl_text_view {
    ($($ty:ty),*) => { $(
        impl<'scope> View<'scope> for $ty {
            fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
                context.mount_text(self)
            }
        }
    )* };
}

impl_text_view!(String);

impl<'scope> View<'scope> for &'scope str {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        context.mount_text(self)
    }
}

impl<'scope> View<'scope> for Cow<'scope, str> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        context.mount_text(self.as_ref())
    }
}

macro_rules! impl_primitive_view {
    ($($ty:ty),*) => { $(
        impl<'scope> View<'scope> for $ty {
            fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
                context.mount_text(&self.to_string())
            }
        }
    )* };
}

impl_primitive_view!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64, bool, char
);

impl<'scope> View<'scope> for () {
    fn mount(&self, _context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        Ok(MountInstance::from_nodes(Vec::new()))
    }
}

impl<'scope, V: View<'scope>> View<'scope> for Option<V> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        self.as_ref().map_or_else(
            || Ok(MountInstance::from_nodes(Vec::new())),
            |value| context.mount(value),
        )
    }
}

impl<'scope, V: View<'scope> + 'scope> View<'scope> for Vec<V> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        context.mount_composite(|child_context| {
            for value in self {
                let _ = child_context.mount(value)?;
            }
            Ok(MountInstance::from_nodes(Vec::new()))
        })
    }
}

impl<'scope, V: View<'scope> + 'scope, const N: usize> View<'scope> for [V; N] {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        context.mount_composite(|child_context| {
            for value in self {
                let _ = child_context.mount(value)?;
            }
            Ok(MountInstance::from_nodes(Vec::new()))
        })
    }
}

impl<'scope> View<'scope> for ViewNil {
    fn mount(&self, _context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        Ok(MountInstance::from_nodes(Vec::new()))
    }
}

impl<'scope, H: View<'scope> + 'scope, T: View<'scope> + 'scope> View<'scope> for ViewCons<H, T> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        context.mount_composite(|child_context| {
            let _ = child_context.mount(&self.0)?;
            let _ = child_context.mount(&self.1)?;
            Ok(MountInstance::from_nodes(Vec::new()))
        })
    }
}

#[macro_export]
macro_rules! chain {
    () => { $crate::ViewNil };
    ($head:expr $(,)?) => { $crate::ViewCons($head, $crate::ViewNil) };
    ($head:expr, $($tail:expr),+ $(,)?) => { $crate::ViewCons($head, $crate::chain!($($tail),+)) };
}

impl<'scope, V: View<'scope>> View<'scope> for SilexResult<V> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        match self {
            Ok(value) => context.mount(value),
            Err(error) => Err(error.clone()),
        }
    }
}
