use super::{MountComposite, MountContext, MountInstance, View, ViewCons, ViewNil};
use silex_core::SilexResult;
use std::{borrow::Cow, vec::Vec};

pub(crate) struct MountPrimitive;

impl MountPrimitive {
    pub(crate) fn text<'scope>(
        context: &MountContext<'scope>,
        text: &str,
    ) -> SilexResult<MountInstance<'scope>> {
        let cleanup_dom = context.dom().clone();
        let node = context.dom().create_text(text)?;
        context.target().append_node(&node)?;
        let cleanup_node = node.clone();
        if let Err(error) = context.owner().on_cleanup(
            Box::new(move || {
                if cleanup_dom.parent(&cleanup_node)?.is_some() {
                    cleanup_dom.remove(&cleanup_node)?;
                }
                Ok(())
            }),
            context.error_handler(),
        ) {
            let _ = context.dom().remove(&node);
            return Err(error);
        }
        Ok(MountInstance::from_nodes(vec![node]))
    }
}

impl<'scope> MountContext<'scope> {
    pub(crate) fn mount_composite<F>(&self, mount: F) -> SilexResult<MountInstance<'scope>>
    where
        F: FnOnce(&MountContext<'scope>) -> SilexResult<MountInstance<'scope>>,
    {
        MountComposite::mount(self, mount)
    }

    pub(crate) fn mount_text(&self, text: &str) -> SilexResult<MountInstance<'scope>> {
        MountPrimitive::text(self, text)
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
    () => { $crate::mount::ViewNil };
    ($head:expr $(,)?) => { $crate::mount::ViewCons($head, $crate::mount::ViewNil) };
    ($head:expr, $($tail:expr),+ $(,)?) => { $crate::mount::ViewCons($head, $crate::chain!($($tail),+)) };
}

impl<'scope, V: View<'scope>> View<'scope> for SilexResult<V> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        match self {
            Ok(value) => context.mount(value),
            Err(error) => Err(error.clone()),
        }
    }
}
