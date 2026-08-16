//! Lifetime-aware reactive traits.

use crate::{
    Callback, ErrorHandlerInput, NodeRef, OwnerAccess, Rx, RxInner, RxValueKind, SilexError,
    SilexResult,
    callback::map_callback_error,
    reactivity::{
        Computed, ReactiveSource, ReadSignal, RwSignal, Signal, SignalSlice, StoredValue,
        WriteSignal,
    },
};
use std::fmt::Debug;

/// Values accepted by the scoped runtime.
pub trait RxData {}
impl<T: ?Sized> RxData for T {}

pub trait RxCloneData: Clone {}
impl<T: Clone> RxCloneData for T {}

pub trait RxError: Clone + Debug {}
impl<T: Clone + Debug> RxError for T {}

/// Construct a owner-owned reactive wrapper from an explicit value.
///
/// Unlike [`From`], this trait receives the [`OwnerAccess`] that owns the node.
/// Implementations must create every node, callback, and owner resource from
/// that owner. They must not create a [`crate::Runtime`], a detached owner, or use
/// thread-local runtime state.
pub trait RxFrom<'owner>: Sized {
    type Value: 'owner;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>;
}

/// Convert an existing scoped reactive source or an explicitly supported
/// value into a target reactive wrapper.
///
/// Unlike [`Into`], this trait receives the [`OwnerAccess`] that owns a node created
/// for a constant value. Existing sources are converted without materializing
/// another node, so their runtime provenance remains attached to the original
/// handle.
#[diagnostic::on_unimplemented(
    message = "reactive input must be an existing scoped source or an explicitly supported value",
    note = "constant reactive inputs require a OwnerAccess<'owner> and only the framework-supported value types are accepted"
)]
pub trait ReactiveInput<'owner, Target>: Sized {
    fn into_reactive_input(self, owner: OwnerAccess<'owner>) -> SilexResult<Target>;
}

/// Construct a owner-owned reactive wrapper from its value's default.
///
/// Every [`RxFrom`] implementation automatically implements this trait. The
/// default operation only delegates to [`RxFrom::rx_from`], so it cannot
/// create a [`crate::Runtime`], a detached owner, or thread-local runtime state.
pub trait RxDefault<'owner>: RxFrom<'owner> {
    fn rx_default(owner: OwnerAccess<'owner>) -> SilexResult<Self>
    where
        Self::Value: Default,
    {
        Self::rx_from(owner, Self::Value::default())
    }
}

impl<'owner, T> RxDefault<'owner> for T where T: RxFrom<'owner> {}

impl<'owner, T: 'owner> RxFrom<'owner> for Signal<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.stored(value.into()).map(Into::into)
    }
}

pub trait RxValue {
    type Value: ?Sized;
}

/// Closure-based tracked and untracked access. No reference can outlive the
/// callback supplied to these methods.
pub trait RxRead: RxValue {
    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U>;

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U>;
}

/// Exposes the runtime provenance retained by a scoped reactive source.
pub trait RuntimeScoped {
    fn owner_access(&self) -> OwnerAccess<'_>;
}

impl<'owner, T, M> RuntimeScoped for Rx<'owner, T, M> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for ReadSignal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for WriteSignal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for RwSignal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.read.owner_access()
    }
}

impl<'owner, T> RuntimeScoped for Computed<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for StoredValue<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'owner, T> RuntimeScoped for Signal<'owner, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.rx.owner_access()
    }
}

impl<S, F, O: ?Sized> RuntimeScoped for SignalSlice<S, F, O>
where
    S: RuntimeScoped,
{
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.source.owner_access()
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for ReadSignal<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.signal(value.into()).map(|(read, _)| read)
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for RwSignal<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.rw_signal(value.into())
    }
}

impl<'owner, T: Clone + PartialEq + 'owner> RxFrom<'owner> for Computed<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        let value = value.into();
        let handler = owner.error_handler(|_: SilexError| {
            unreachable!("constant computed cannot report a user error")
        })?;
        owner.computed(move || Ok::<T, SilexError>(value.clone()), handler)
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for StoredValue<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.stored(value.into())
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Rx<'owner, T, RxValueKind> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.constant(value.into())
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Callback<'owner, T> {
    type Value = ();

    fn rx_from<V>(owner: OwnerAccess<'owner>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.callback(|_: T| Ok::<(), SilexError>(()))
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for NodeRef<'owner, T> {
    type Value = ();

    fn rx_from<V>(owner: OwnerAccess<'owner>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.node_ref()
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for RwSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for StoredValue<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Signal<'owner, T>> for Rx<'owner, T, RxValueKind> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Signal<'owner, T>> {
        Ok(self.into_signal())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for RwSignal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self.read_signal())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, RwSignal<'owner, T>> for RwSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<RwSignal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Computed<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Computed<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, StoredValue<'owner, T>> for StoredValue<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<StoredValue<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for RwSignal<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Computed<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for StoredValue<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self.into_rx())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Rx<'owner, T, RxValueKind> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self)
    }
}

macro_rules! impl_reactive_input_values {
    ($($value:ty),* $(,)?) => {
        $(
            impl<'owner> ReactiveInput<'owner, Signal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $value>> {
                    <Signal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, ReadSignal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $value>> {
                    <ReadSignal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, RwSignal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<RwSignal<'owner, $value>> {
                    <RwSignal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Computed<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Computed<'owner, $value>> {
                    <Computed<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, StoredValue<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<StoredValue<'owner, $value>> {
                    <StoredValue<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Rx<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Rx<'owner, $value>> {
                    <Rx<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }
        )*
    };
}

impl_reactive_input_values!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
);

macro_rules! impl_reactive_input_str_values {
    ($($target:ty),* $(,)?) => {
        $(
            impl<'owner, 'value> ReactiveInput<'owner, Signal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $target>> {
                    <Signal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, ReadSignal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $target>> {
                    <ReadSignal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, RwSignal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<RwSignal<'owner, $target>> {
                    <RwSignal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Computed<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Computed<'owner, $target>> {
                    <Computed<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, StoredValue<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<StoredValue<'owner, $target>> {
                    <StoredValue<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Rx<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Rx<'owner, $target>> {
                    <Rx<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }
        )*
    };
}

impl_reactive_input_str_values!(String);

/// Clone-based convenience access built on top of [`RxRead`].
pub trait RxGet: RxRead
where
    Self::Value: Sized + Clone,
{
    fn get_untracked(&self) -> SilexResult<Self::Value> {
        self.with_untracked(Clone::clone)
    }

    fn get(&self) -> SilexResult<Self::Value> {
        self.with(Clone::clone)
    }
}

impl<T> RxGet for T
where
    T: RxRead + ?Sized,
    T::Value: Sized + Clone,
{
}

/// Unified scoped writes.
pub trait RxWrite: RxValue {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> SilexResult<U>;

    fn rx_notify(&self) -> SilexResult<()>;

    fn update<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> SilexResult<U> {
        self.rx_update_untracked(f)
    }

    fn set(&self, value: Self::Value) -> SilexResult<()>
    where
        Self::Value: Sized,
    {
        self.update(|current| *current = value)
    }

    fn update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> SilexResult<U> {
        self.rx_update_untracked(f)
    }

    fn set_untracked(&self, value: Self::Value) -> SilexResult<()>
    where
        Self::Value: Sized,
    {
        self.update_untracked(|current| *current = value)
    }

    fn notify(&self) -> SilexResult<()> {
        self.rx_notify()
    }

    fn setter(self, value: Self::Value) -> impl Fn() -> SilexResult<()> + Clone
    where
        Self: Sized + Clone,
        Self::Value: Sized + Clone,
    {
        move || self.set(value.clone())
    }

    fn updater<F>(self, f: F) -> impl Fn() -> SilexResult<()> + Clone
    where
        Self: Sized + Clone,
        Self::Value: Sized,
        F: Fn(&mut Self::Value) + Clone,
    {
        move || self.update(f.clone())
    }
}

impl<'owner, T: 'owner> RxValue for ReadSignal<'owner, T> {
    type Value = T;
}

impl<'owner, T: 'owner> RxRead for ReadSignal<'owner, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(SilexError::fatal)
    }
}

impl<'owner, T: 'owner> RxValue for WriteSignal<'owner, T> {
    type Value = T;
}

impl<'owner, T: 'owner> RxWrite for WriteSignal<'owner, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        self.inner.notify().map_err(SilexError::fatal)
    }
}

impl<'owner, T: 'owner> RxValue for RwSignal<'owner, T> {
    type Value = T;
}

impl<'owner, T: 'owner> RxRead for RwSignal<'owner, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.read.with(f)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.read.with_untracked(f)
    }
}

impl<'owner, T: 'owner> RxWrite for RwSignal<'owner, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.write.rx_update_untracked(f)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        self.write.rx_notify()
    }
}

impl<'owner, T: 'owner> RxValue for Signal<'owner, T> {
    type Value = T;
}

impl<'owner, T: 'owner> RxRead for Signal<'owner, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.rx.with(f)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.rx.with_untracked(f)
    }
}

impl<'owner, T: 'owner> RxValue for Rx<'owner, T, RxValueKind> {
    type Value = T;
}

impl<'owner, T: 'owner> RxRead for Rx<'owner, T, RxValueKind> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.with(f).map_err(SilexError::fatal),
            RxInner::Computed(computed) => computed.with(f).map_err(map_callback_error),
            RxInner::Stored(stored) => stored.with(f).map_err(SilexError::fatal),
        }
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.with_untracked(f).map_err(SilexError::fatal),
            RxInner::Computed(computed) => computed.with_untracked(f).map_err(map_callback_error),
            RxInner::Stored(stored) => stored.with(f).map_err(SilexError::fatal),
        }
    }
}

impl<'owner, T: 'owner> RxValue for StoredValue<'owner, T> {
    type Value = T;
}

impl<'owner, T: 'owner> RxRead for StoredValue<'owner, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }
}

impl<'owner, T: 'owner> RxWrite for StoredValue<'owner, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        Ok(())
    }
}

impl<'owner, T: 'owner> RxValue for Computed<'owner, T> {
    type Value = T;
}

macro_rules! impl_primitive_rx_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RxValue for $ty {
                type Value = Self;
            }
        )*
    };
}

impl_primitive_rx_value!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
);

impl RxValue for &str {
    type Value = String;
}

impl<'owner, T: 'owner> RxRead for Computed<'owner, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(map_callback_error)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(map_callback_error)
    }
}

impl<T> RxValue for (T,)
where
    T: RxValue,
    T::Value: Sized + RxData,
{
    type Value = (T::Value,);
}

macro_rules! impl_tuple_rx_value {
    ($($name:ident),+ $(,)?) => {
        impl<$($name),+> RxValue for ($($name,)+)
        where
            $($name: RxValue, $name::Value: Sized + RxData),+
        {
            type Value = ($($name::Value,)+);
        }
    };
}

impl_tuple_rx_value!(A, B);
impl_tuple_rx_value!(A, B, C);
impl_tuple_rx_value!(A, B, C, D);
impl_tuple_rx_value!(A, B, C, D, E);
impl_tuple_rx_value!(A, B, C, D, E, F);

/// Aggregate dependency tracking and clone-backed reads for reactive tuples.
///
/// Tracking only borrows each member briefly and therefore does not require the
/// member values to implement [`Clone`]. Aggregate reads materialize an owned
/// tuple before invoking the callback, so those implementations intentionally
/// require cloneable member values. [`RxGet`] is provided automatically by its
/// blanket implementation; tuples are not [`RxWrite`] values because updating
/// multiple independent sources cannot provide a transactional mutation.
macro_rules! impl_tuple_rx_traits {
    ($($name:ident : $index:tt),+ $(,)?) => {
        impl<$($name),+> RxRead for ($($name,)+)
        where
            $($name: RxRead, $name::Value: Sized + Clone + RxData),+
        {
            fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
                let value = ($(self.$index.with(|value| (*value).clone())?,)+);
                Ok(f(&value))
            }

            fn with_untracked<U>(
                &self,
                f: impl FnOnce(&Self::Value) -> U,
            ) -> SilexResult<U> {
                let value = ($(self.$index.with_untracked(|value| (*value).clone())?,)+);
                Ok(f(&value))
            }
        }
    };
}

impl_tuple_rx_traits!(A: 0);
impl_tuple_rx_traits!(A: 0, B: 1);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2, D: 3);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2, D: 3, E: 4);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

/// Reactive helpers for `Option<T>` values.
pub trait RxOptionExt<T>: RxRead<Value = Option<T>> + Clone {
    fn map_or<'owner, U, H>(
        &self,
        owner: OwnerAccess<'owner>,
        default: U,
        f: impl Fn(&T) -> U + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, U>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        U: Clone + 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed_always(
                move || {
                    source.with(|value| value.as_ref().map(&f).unwrap_or_else(|| default.clone()))
                },
                error_handler,
            )
            .map(Computed::into_rx)
    }

    fn unwrap_or<'owner, H>(
        &self,
        owner: OwnerAccess<'owner>,
        default: T,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, T>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        T: PartialEq + Clone + 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed(
                move || {
                    source.with(|value| value.as_ref().cloned().unwrap_or_else(|| default.clone()))
                },
                error_handler,
            )
            .map(|computed| computed.into_rx())
    }

    fn map_or_else<'owner, U, H>(
        &self,
        owner: OwnerAccess<'owner>,
        default: impl Fn() -> U + 'owner,
        f: impl Fn(&T) -> U + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, U>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        U: 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed_always(
                move || source.with(|value| value.as_ref().map(&f).unwrap_or_else(&default)),
                error_handler,
            )
            .map(Computed::into_rx)
    }

    fn and_then<'owner, U, H>(
        &self,
        owner: OwnerAccess<'owner>,
        f: impl Fn(&T) -> Option<U> + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, Option<U>>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        U: 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed_always(
                move || source.with(|value| value.as_ref().and_then(&f)),
                error_handler,
            )
            .map(Computed::into_rx)
    }

    fn is_some_and<'owner, H>(
        &self,
        owner: OwnerAccess<'owner>,
        f: impl Fn(&T) -> bool + 'owner,
        error_handler: H,
    ) -> SilexResult<Rx<'owner, bool>>
    where
        Self: ReactiveSource<'owner> + 'owner,
        T: 'owner,
        H: ErrorHandlerInput<'owner>,
    {
        let error_handler = error_handler.handler_ref();
        let source = owner.promote(self.clone(), error_handler)?;
        owner
            .computed(
                move || source.with(|value| value.as_ref().is_some_and(&f)),
                error_handler,
            )
            .map(|computed| computed.into_rx())
    }

    fn if_some_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<Option<U>> {
        self.with_untracked(|value| value.as_ref().map(f))
    }
}

impl<S, T> RxOptionExt<T> for S where S: RxRead<Value = Option<T>> + Clone {}

/// Sources used by list rendering helpers.
pub trait ForLoopSource {
    type Item: Clone;
    fn as_slice(&self) -> SilexResult<&[Self::Item]>;
}

impl<T: Clone> ForLoopSource for Vec<T> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self)
    }
}

impl<T: Clone> ForLoopSource for Option<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self.as_deref().unwrap_or_default())
    }
}

impl<T: Clone> ForLoopSource for SilexResult<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Ok(items) => Ok(items.as_slice()),
            Err(error) => Err(error.clone()),
        }
    }
}
