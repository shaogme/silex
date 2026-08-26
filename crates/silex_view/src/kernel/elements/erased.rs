use super::untyped::Element;
use crate::kernel::{MountContext, MountInstance, View, ViewCons, ViewNil};
use silex_core::SilexResult;
use std::{
    fmt::{Debug, Formatter, Result},
    rc::Rc,
};

/// owner-bound type-erased View。
#[derive(Default)]
pub enum AnyView<'scope> {
    #[default]
    Empty,
    Text(String),
    Element(Element<'scope>),
    List(Vec<AnyView<'scope>>),
    Boxed(Rc<dyn View<'scope> + 'scope>),
}

impl<'scope> AnyView<'scope> {
    pub fn new<V>(view: V) -> Self
    where
        V: View<'scope> + 'scope,
    {
        Self::Boxed(Rc::new(view))
    }

    pub fn into_any(self) -> Self {
        self
    }
}

fn mount_list<'scope>(
    list: &[AnyView<'scope>],
    context: &MountContext<'scope>,
) -> SilexResult<MountInstance<'scope>> {
    context.mount_composite(move |child_context| {
        for child in list {
            let _ = child_context.mount(child)?;
        }
        Ok(MountInstance::from_nodes(Vec::new()))
    })
}

impl<'scope> View<'scope> for AnyView<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        match self {
            Self::Empty => Ok(MountInstance::from_nodes(Vec::new())),
            Self::Text(text) => context.mount(text),
            Self::Element(element) => context.mount(element),
            Self::List(list) => mount_list(list, context),
            Self::Boxed(view) => context.mount(view.as_ref()),
        }
    }
}

impl<'scope> Clone for AnyView<'scope> {
    fn clone(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Text(text) => Self::Text(text.clone()),
            Self::Element(element) => Self::Element(element.clone()),
            Self::List(list) => Self::List(list.clone()),
            Self::Boxed(view) => Self::Boxed(view.clone()),
        }
    }
}

impl PartialEq for AnyView<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Empty, Self::Empty) => true,
            (Self::Text(left), Self::Text(right)) => left == right,
            (Self::Element(left), Self::Element(right)) => left == right,
            (Self::List(left), Self::List(right)) => left == right,
            (Self::Boxed(left), Self::Boxed(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Debug for AnyView<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        match self {
            Self::Empty => formatter.write_str("AnyView(Empty)"),
            Self::Text(text) => formatter.debug_tuple("AnyView(Text)").field(text).finish(),
            Self::Element(_) => formatter.write_str("AnyView(Element)"),
            Self::List(list) => formatter
                .debug_tuple("AnyView(List)")
                .field(&list.len())
                .finish(),
            Self::Boxed(_) => formatter.write_str("AnyView(Boxed)"),
        }
    }
}

impl<'scope> From<Element<'scope>> for AnyView<'scope> {
    fn from(value: Element<'scope>) -> Self {
        Self::Element(value)
    }
}

impl<'scope> From<String> for AnyView<'scope> {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl<'scope> From<&str> for AnyView<'scope> {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl<'scope> From<()> for AnyView<'scope> {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

macro_rules! impl_from_primitive {
    ($($ty:ty),*) => { $(
        impl<'scope> From<$ty> for AnyView<'scope> {
            fn from(value: $ty) -> Self { Self::Text(value.to_string()) }
        }
    )* };
}

impl_from_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl<'scope, V> From<Vec<V>> for AnyView<'scope>
where
    V: View<'scope> + 'scope,
{
    fn from(value: Vec<V>) -> Self {
        Self::List(value.into_iter().map(View::into_any).collect())
    }
}

impl<'scope, V> From<Option<V>> for AnyView<'scope>
where
    V: View<'scope> + 'scope,
{
    fn from(value: Option<V>) -> Self {
        value.map_or(Self::Empty, AnyView::new)
    }
}

impl<'scope> From<ViewNil> for AnyView<'scope> {
    fn from(_: ViewNil) -> Self {
        Self::Empty
    }
}

impl<'scope, H, T> From<ViewCons<H, T>> for AnyView<'scope>
where
    H: View<'scope> + 'scope,
    T: View<'scope> + 'scope,
{
    fn from(value: ViewCons<H, T>) -> Self {
        Self::new(value)
    }
}

#[macro_export]
macro_rules! view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $( $pat $(if $guard)? => $val.into_any(), )*
        }
    };
}
