use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::{ApplyAttributes, View, ViewCons, ViewNil, ViewOwner};
use std::rc::Rc;
use web_sys::Node;

pub type RenderThunk<'scope, 'run> =
    silex_vtable::thunk::ThunkBox<'scope, (Node, Vec<PendingAttribute<'scope, 'run>>), ()>;

/// Scope-bound type-erased view.
#[derive(Default)]
pub enum AnyView<'scope, 'run> {
    #[default]
    Empty,
    Text(String),
    Element(Element<'scope, 'run>),
    List(Vec<AnyView<'scope, 'run>>),
    Boxed(
        Rc<dyn View<'scope, 'run> + 'scope>,
        Vec<PendingAttribute<'scope, 'run>>,
    ),
}

impl<'scope, 'run> AnyView<'scope, 'run> {
    pub fn new<V>(view: V) -> Self
    where
        V: View<'scope, 'run> + 'scope,
    {
        Self::Boxed(Rc::new(view), Vec::new())
    }

    pub fn into_any(self) -> Self {
        self
    }
}

fn merge_attrs<'scope, 'run>(
    mut inner_attrs: Vec<PendingAttribute<'scope, 'run>>,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
) -> Vec<PendingAttribute<'scope, 'run>> {
    inner_attrs.extend(attrs);
    crate::attribute::consolidate_attributes(inner_attrs)
}

fn mount_list<'scope, 'run>(
    list: &[AnyView<'scope, 'run>],
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
) {
    for (index, child) in list.iter().enumerate() {
        child.mount(
            owner,
            parent,
            if index == 0 {
                attrs.clone()
            } else {
                Vec::new()
            },
        );
    }
}

fn mount_list_owned<'scope, 'run>(
    list: Vec<AnyView<'scope, 'run>>,
    owner: &dyn ViewOwner<'scope, 'run>,
    parent: &Node,
    attrs: Vec<PendingAttribute<'scope, 'run>>,
) {
    for (index, child) in list.into_iter().enumerate() {
        child.mount_owned(
            owner,
            parent,
            if index == 0 {
                attrs.clone()
            } else {
                Vec::new()
            },
        );
    }
}

impl<'scope, 'run> ApplyAttributes<'scope, 'run> for AnyView<'scope, 'run> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        match self {
            Self::Empty | Self::Text(_) => {}
            Self::Element(element) => element.apply_attributes(attrs),
            Self::List(list) => {
                for child in list {
                    child.apply_attributes(attrs.clone());
                }
            }
            Self::Boxed(_, inner_attrs) => {
                let current = std::mem::take(inner_attrs);
                *inner_attrs = merge_attrs(current, attrs);
            }
        }
    }
}

impl<'scope, 'run> View<'scope, 'run> for AnyView<'scope, 'run> {
    fn into_any(self) -> Self {
        self
    }

    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        match self {
            Self::Empty => {}
            Self::Text(text) => text.mount(owner, parent, attrs),
            Self::Element(element) => element.mount(owner, parent, attrs),
            Self::List(list) => mount_list(list, owner, parent, attrs),
            Self::Boxed(view, inner_attrs) => {
                view.mount(owner, parent, merge_attrs(inner_attrs.clone(), attrs));
            }
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        match self {
            Self::Empty => {}
            Self::Text(text) => text.mount_owned(owner, parent, attrs),
            Self::Element(element) => element.mount_owned(owner, parent, attrs),
            Self::List(list) => mount_list_owned(list, owner, parent, attrs),
            Self::Boxed(view, inner_attrs) => {
                view.mount(owner, parent, merge_attrs(inner_attrs, attrs));
            }
        }
    }
}

impl<'scope, 'run> Clone for AnyView<'scope, 'run> {
    fn clone(&self) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Text(text) => Self::Text(text.clone()),
            Self::Element(element) => Self::Element(element.clone()),
            Self::List(list) => Self::List(list.clone()),
            Self::Boxed(view, attrs) => Self::Boxed(view.clone(), attrs.clone()),
        }
    }
}

impl PartialEq for AnyView<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Empty, Self::Empty) => true,
            (Self::Text(left), Self::Text(right)) => left == right,
            (Self::Element(left), Self::Element(right)) => left == right,
            (Self::List(left), Self::List(right)) => left == right,
            (Self::Boxed(left, left_attrs), Self::Boxed(right, right_attrs)) => {
                Rc::ptr_eq(left, right) && left_attrs == right_attrs
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for AnyView<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("AnyView(Empty)"),
            Self::Text(text) => f.debug_tuple("AnyView(Text)").field(text).finish(),
            Self::Element(_) => f.write_str("AnyView(Element)"),
            Self::List(list) => f.debug_tuple("AnyView(List)").field(&list.len()).finish(),
            Self::Boxed(_, _) => f.write_str("AnyView(Boxed)"),
        }
    }
}

impl<'scope, 'run> From<Element<'scope, 'run>> for AnyView<'scope, 'run> {
    fn from(value: Element<'scope, 'run>) -> Self {
        Self::Element(value)
    }
}

impl<'scope, 'run> From<String> for AnyView<'scope, 'run> {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl<'scope, 'run> From<&str> for AnyView<'scope, 'run> {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl<'scope, 'run> From<()> for AnyView<'scope, 'run> {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

macro_rules! impl_from_primitive {
    ($($ty:ty),*) => {
        $(
            impl<'scope, 'run> From<$ty> for AnyView<'scope, 'run> {
                fn from(value: $ty) -> Self {
                    Self::Text(value.to_string())
                }
            }
        )*
    };
}

impl_from_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl<'scope, 'run, V> From<Vec<V>> for AnyView<'scope, 'run>
where
    V: View<'scope, 'run> + 'scope,
{
    fn from(value: Vec<V>) -> Self {
        Self::List(value.into_iter().map(View::into_any).collect())
    }
}

impl<'scope, 'run, V> From<Option<V>> for AnyView<'scope, 'run>
where
    V: View<'scope, 'run> + 'scope,
{
    fn from(value: Option<V>) -> Self {
        value.map_or(Self::Empty, AnyView::new)
    }
}

impl<'scope, 'run> From<ViewNil> for AnyView<'scope, 'run> {
    fn from(_: ViewNil) -> Self {
        Self::Empty
    }
}

impl<'scope, 'run, H, T> From<ViewCons<H, T>> for AnyView<'scope, 'run>
where
    H: View<'scope, 'run> + 'scope,
    T: View<'scope, 'run> + 'scope,
{
    fn from(value: ViewCons<H, T>) -> Self {
        Self::new(value)
    }
}

#[macro_export]
macro_rules! view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $val.into_any(),
            )*
        }
    };
}

#[macro_export]
macro_rules! any_view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $val.into_any(),
            )*
        }
    };
}
