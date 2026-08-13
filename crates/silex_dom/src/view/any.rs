use super::mount::mount_composite;
use crate::attribute::PendingAttribute;
use crate::element::Element;
use crate::view::{
    ApplyAttributes, MountErrorHandler, MountOwner, View, ViewCons, ViewFactory, ViewNil,
};
use silex_core::SilexResult;
use std::rc::Rc;
use web_sys::Node;

/// Scope-bound type-erased view.
#[derive(Default)]
pub enum AnyView<'scope> {
    #[default]
    Empty,
    Text(String),
    Element(Element<'scope>),
    List(Vec<AnyView<'scope>>),
    Boxed(
        Rc<dyn ViewFactory<'scope> + 'scope>,
        Vec<PendingAttribute<'scope>>,
    ),
}

impl<'scope> AnyView<'scope> {
    pub fn new<V>(view: V) -> Self
    where
        V: ViewFactory<'scope> + 'scope,
    {
        Self::Boxed(Rc::new(view), Vec::new())
    }

    pub fn into_any(self) -> Self {
        self
    }
}

fn merge_attrs<'scope>(
    mut inner_attrs: Vec<PendingAttribute<'scope>>,
    attrs: Vec<PendingAttribute<'scope>>,
) -> Vec<PendingAttribute<'scope>> {
    inner_attrs.extend(attrs);
    crate::attribute::consolidate_attributes(inner_attrs)
}

fn mount_list<'scope>(
    list: &[AnyView<'scope>],
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
            for (index, child) in list.iter().enumerate() {
                let _ = child.create_mount_instance(
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

fn mount_list_owned<'scope>(
    list: Vec<AnyView<'scope>>,
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
            for (index, child) in list.into_iter().enumerate() {
                let _ = child.create_mount_instance(
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

impl<'scope> ApplyAttributes<'scope> for AnyView<'scope> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
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

impl<'scope> View<'scope> for AnyView<'scope> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        match self {
            Self::Empty => Ok(()),
            Self::Text(text) => text.mount(owner, parent, attrs, error_handler),
            Self::Element(element) => element.mount(owner, parent, attrs, error_handler),
            Self::List(list) => mount_list(list, owner, parent, attrs, error_handler),
            Self::Boxed(view, inner_attrs) => view
                .create_mount_instance(
                    owner,
                    parent,
                    merge_attrs(inner_attrs.clone(), attrs),
                    error_handler,
                )
                .map(|_| ()),
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
            Self::Empty => Ok(()),
            Self::Text(text) => text.mount_owned(owner, parent, attrs, error_handler),
            Self::Element(element) => element.mount_owned(owner, parent, attrs, error_handler),
            Self::List(list) => mount_list_owned(list, owner, parent, attrs, error_handler),
            Self::Boxed(view, inner_attrs) => view
                .create_mount_instance(
                    owner,
                    parent,
                    merge_attrs(inner_attrs, attrs),
                    error_handler,
                )
                .map(|_| ()),
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
            Self::Boxed(view, attrs) => Self::Boxed(view.clone(), attrs.clone()),
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
            (Self::Boxed(left, left_attrs), Self::Boxed(right, right_attrs)) => {
                Rc::ptr_eq(left, right) && left_attrs == right_attrs
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for AnyView<'_> {
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
    ($($ty:ty),*) => {
        $(
            impl<'scope> From<$ty> for AnyView<'scope> {
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

impl<'scope, V> From<Vec<V>> for AnyView<'scope>
where
    V: ViewFactory<'scope> + 'scope,
{
    fn from(value: Vec<V>) -> Self {
        Self::List(value.into_iter().map(ViewFactory::into_any).collect())
    }
}

impl<'scope, V> From<Option<V>> for AnyView<'scope>
where
    V: ViewFactory<'scope> + 'scope,
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
            $(
                $pat $(if $guard)? => $val.into_any(),
            )*
        }
    };
}
