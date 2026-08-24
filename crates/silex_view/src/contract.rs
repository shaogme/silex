use crate::any::AnyView;
use crate::context::MountContext;
use silex_core::{
    ErrorHandlerInput, OwnerAccess, ReactiveSource, Rx, RxData, RxValue, SilexResult,
};
use silex_dom::DomNode;
use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    ops::{Add, Deref, Div, Mul, Sub},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropMissing;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropFixed;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewNil;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewCons<H, T>(pub H, pub T);

/// generated builder 使用的 borrowed/owned prop 包装。
pub enum Prop<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T: RxValue> Prop<'a, T> {
    pub fn new_borrowed(value: &'a T) -> Self {
        Self::Borrowed(value)
    }

    pub fn new_owned(value: T) -> Self {
        Self::Owned(value)
    }

    pub fn new(value: &'a T) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a, T: Clone> Prop<'a, T> {
    pub fn into_owned(self) -> T {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value.clone(),
        }
    }
}

impl<'a, T: Clone> Clone for Prop<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(value) => Self::Owned(value.clone()),
            Self::Borrowed(value) => Self::Owned((*value).clone()),
        }
    }
}

impl<'a, T: Copy> Copy for Prop<'a, T> {}

pub trait PropInto<T> {
    fn prop_into(self) -> T;
}

impl<T> PropInto<T> for T {
    fn prop_into(self) -> T {
        self
    }
}

impl<'a, T: Clone> PropInto<T> for Prop<'a, T> {
    fn prop_into(self) -> T {
        self.into_owned()
    }
}

impl<'scope, 'a, T> View<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: View<'scope>,
{
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        match self {
            Self::Owned(value) => context.mount(value),
            Self::Borrowed(value) => context.mount(*value),
        }
    }
}

impl<'a, T: RxValue> RxValue for Prop<'a, T> {
    type Owned = T::Owned;
}

impl<'a, T> Prop<'a, T> {
    pub fn promote<'scope, H>(
        self,
        owner: OwnerAccess<'scope>,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, T::Owned>>
    where
        'a: 'scope,
        T: ReactiveSource<'scope> + Clone,
        T::Owned: Sized + RxData + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        owner.promote(self.into_owned(), error_handler)
    }
}

impl<'a, T> Deref for Prop<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(value) => value,
            Self::Borrowed(value) => value,
        }
    }
}

impl<'a, T: Debug> Debug for Prop<'a, T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(formatter)
    }
}

impl<'a, T: Display> Display for Prop<'a, T> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(formatter)
    }
}

macro_rules! impl_forward_binop_copy {
    ($trait:ident, $method:ident) => {
        impl<'a, T, Rhs> $trait<Rhs> for Prop<'a, T>
        where
            T: Copy + $trait<Rhs>,
        {
            type Output = <T as $trait<Rhs>>::Output;

            fn $method(self, rhs: Rhs) -> Self::Output {
                self.deref().$method(rhs)
            }
        }
    };
}

impl_forward_binop_copy!(Add, add);
impl_forward_binop_copy!(Sub, sub);
impl_forward_binop_copy!(Mul, mul);
impl_forward_binop_copy!(Div, div);

/// 一次 View mount 产生的节点快照；它不拥有 owner。
#[must_use = "a MountInstance represents a live DOM mount"]
pub struct MountInstance<'scope> {
    nodes: Vec<DomNode>,
    marker: PhantomData<&'scope ()>,
}

impl<'scope> MountInstance<'scope> {
    pub fn from_nodes(nodes: Vec<DomNode>) -> Self {
        Self {
            nodes,
            marker: PhantomData,
        }
    }

    pub fn nodes(&self) -> &[DomNode] {
        &self.nodes
    }

    pub fn first_node(&self) -> Option<&DomNode> {
        self.nodes.first()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn into_nodes(self) -> Vec<DomNode> {
        self.nodes
    }
}

/// 可重复执行的 View 工厂契约。
pub trait View<'scope> {
    fn into_any(self) -> AnyView<'scope>
    where
        Self: Sized + 'scope,
    {
        AnyView::new(self)
    }

    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>>;
}
