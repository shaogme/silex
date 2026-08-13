use super::any::AnyView;
use super::owner::{MountErrorHandler, MountOwner};
use crate::attribute::PendingAttribute;
use silex_core::{ErrorReporter, ReactiveSource, Rx, RxData, RxValue, Scope, SilexResult};
use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    ops::{Add, Deref, Div, Mul, Sub},
};
use web_sys::Node;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropMissing;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PropFixed;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewNil;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewCons<H, T>(pub H, pub T);

/// Apply attributes to a view while preserving their scope boundary.
pub trait ApplyAttributes<'scope> {
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute<'scope>>) {}
}

/// Component prop wrapper used by generated builders.
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

impl<'scope, 'a, T> ApplyAttributes<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: ApplyAttributes<'scope>,
{
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        match self {
            Self::Owned(value) => value.apply_attributes(attrs),
            Self::Borrowed(value) => {
                let _ = (value, attrs);
            }
        }
    }
}

impl<'scope, 'a, T> View<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: View<'scope>,
{
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()> {
        match self {
            Self::Owned(value) => value.mount(owner, parent, attrs, error_handler),
            Self::Borrowed(value) => value.mount(owner, parent, attrs, error_handler),
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
            Self::Owned(value) => value.mount_owned(owner, parent, attrs, error_handler),
            Self::Borrowed(value) => value.mount(owner, parent, attrs, error_handler),
        }
    }
}

impl<'a, T: RxValue> RxValue for Prop<'a, T> {
    type Value = T::Value;
}

impl<'a, T> Prop<'a, T> {
    pub fn promote<'scope>(
        self,
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<Rx<'scope, T::Value>>
    where
        'a: 'scope,
        T: ReactiveSource<'scope> + Clone,
        T::Value: Sized + RxData + 'scope,
    {
        scope.promote(self.into_owned(), error_handler)
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
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
    }
}

impl<'a, T: Display> Display for Prop<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.deref().fmt(f)
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

/// View conversion and mounting contract.
pub trait View<'scope> {
    fn into_any(self) -> AnyView<'scope>
    where
        Self: Sized + 'scope,
    {
        AnyView::new(self)
    }

    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>;

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized;
}
