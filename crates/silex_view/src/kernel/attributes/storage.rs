use super::{
    apply_to_dom::ApplyToDom,
    binding::ReactiveBinding,
    model::{ApplyTarget, Attr},
    operation::AttrOp,
};
use crate::kernel::Prop;
use silex_core::{
    Rx,
    reactivity::{Computed, ReadSignal, Signal, StoredValue},
};
use std::borrow::Cow;
#[derive(Clone, Default)]
pub struct AttributeGroup<'scope>(Vec<AttrOp<'scope>>);

impl<'scope> AttributeGroup<'scope> {
    pub fn new(ops: Vec<AttrOp<'scope>>) -> Self {
        Self(ops)
    }

    pub fn as_ops(&self) -> &[AttrOp<'scope>] {
        &self.0
    }

    pub fn into_ops(self) -> Vec<AttrOp<'scope>> {
        self.0
    }
}

pub fn group<'scope, I>(items: I) -> AttributeGroup<'scope>
where
    I: IntoIterator,
    I::Item: ApplyToDom<'scope> + 'scope,
{
    AttributeGroup::new(
        items
            .into_iter()
            .map(|item| item.into_op(ApplyTarget::Apply))
            .collect(),
    )
}

pub trait IntoStorable<'scope> {
    type Stored: ApplyToDom<'scope> + 'scope;
    fn into_storable(self) -> Self::Stored;
}
impl<'scope, 'a: 'scope> IntoStorable<'scope> for &'a str {
    type Stored = &'a str;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope> IntoStorable<'scope> for &String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self.clone()
    }
}
impl<'scope> IntoStorable<'scope> for String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, 'a: 'scope> IntoStorable<'scope> for Cow<'a, str> {
    type Stored = Cow<'a, str>;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope> IntoStorable<'scope> for bool {
    type Stored = bool;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
macro_rules! impl_storable { ($($ty:ty),*) => { $(impl<'scope> IntoStorable<'scope> for $ty { type Stored = $ty; fn into_storable(self) -> Self::Stored { self } })* }; }
impl_storable!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);
impl<'scope, T> IntoStorable<'scope> for Rx<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, T> IntoStorable<'scope> for ReadSignal<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope, T> IntoStorable<'scope> for Signal<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope, T> IntoStorable<'scope> for Computed<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope, T> IntoStorable<'scope> for StoredValue<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T>;
    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}
impl<'scope> IntoStorable<'scope> for Attr<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope> IntoStorable<'scope> for AttrOp<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, K, V> IntoStorable<'scope> for (K, V)
where
    K: IntoStorable<'scope>,
    V: IntoStorable<'scope>,
    (K::Stored, V::Stored): ApplyToDom<'scope> + 'scope,
{
    type Stored = (K::Stored, V::Stored);
    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_storable())
    }
}
impl<'scope, V: IntoStorable<'scope>, const N: usize> IntoStorable<'scope> for [V; N]
where
    V::Stored: 'scope,
{
    type Stored = [V::Stored; N];
    fn into_storable(self) -> Self::Stored {
        self.map(IntoStorable::into_storable)
    }
}
impl<'scope, V: IntoStorable<'scope>> IntoStorable<'scope> for Option<V>
where
    V::Stored: 'scope,
{
    type Stored = Option<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.map(IntoStorable::into_storable)
    }
}
impl<'scope, V: IntoStorable<'scope>> IntoStorable<'scope> for Vec<V>
where
    V::Stored: 'scope,
{
    type Stored = Vec<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.into_iter().map(IntoStorable::into_storable).collect()
    }
}
impl<'scope> IntoStorable<'scope> for AttributeGroup<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
impl<'scope, 'a, T> IntoStorable<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: Clone + IntoStorable<'scope>,
    T::Stored: 'scope,
{
    type Stored = T::Stored;
    fn into_storable(self) -> Self::Stored {
        match self {
            Prop::Owned(value) => value.into_storable(),
            Prop::Borrowed(value) => value.clone().into_storable(),
        }
    }
}
