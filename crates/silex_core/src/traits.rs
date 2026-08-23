//! Lifetime-aware reactive traits.

use crate::{
    Callback, ErrorHandlerInput, NodeRef, OwnerAccess, Rx, SilexError, SilexResult,
    reactivity::{Computed, ReactiveSource, ReadSignal, Signal, StoredValue},
};
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

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

pub trait RxValue {
    type Value: ?Sized;
}

/// Establish a dependency without borrowing or cloning the source payload.
pub trait RxBase: RxValue {
    fn track(&self) -> SilexResult<()>;
}

/// Closure-based tracked and untracked access. No reference can outlive the
/// callback supplied to these methods.
pub trait RxRead: RxBase {
    type ReadGuard<'a>: Deref<Target = Self::Value>
    where
        Self: 'a;

    fn read(&self) -> SilexResult<Self::ReadGuard<'_>>;

    fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>>;

    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U>;

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U>;
}

/// Exposes the runtime provenance retained by a scoped reactive source.
pub trait RuntimeScoped {
    fn owner_access(&self) -> OwnerAccess<'_>;
}

impl<'owner, T: 'owner> RxFrom<'owner> for ReadSignal<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.signal(value.into()).map(Into::into)
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for Signal<'owner, T> {
    type Value = T;

    fn rx_from<V>(owner: OwnerAccess<'owner>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        owner.signal(value.into())
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

impl<'owner, T: 'owner> RxFrom<'owner> for Rx<'owner, T> {
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

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for ReadSignal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self)
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, ReadSignal<'owner, T>> for Signal<'owner, T> {
    fn into_reactive_input(
        self,
        _scope: OwnerAccess<'owner>,
    ) -> SilexResult<ReadSignal<'owner, T>> {
        Ok(self.into())
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
        Ok(self.into())
    }
}

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for ReadSignal<'owner, T> {
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

impl<'owner, T: 'owner> ReactiveInput<'owner, Rx<'owner, T>> for Rx<'owner, T> {
    fn into_reactive_input(self, _scope: OwnerAccess<'owner>) -> SilexResult<Rx<'owner, T>> {
        Ok(self)
    }
}

macro_rules! impl_reactive_input_values {
    ($($value:ty),* $(,)?) => {
        $(
            impl<'owner> ReactiveInput<'owner, ReadSignal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $value>> {
                    <ReadSignal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Signal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $value>> {
                    <Signal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
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
    type WriteGuard<'a>: DerefMut<Target = Self::Value>
    where
        Self: 'a;

    fn write(&self) -> SilexResult<Self::WriteGuard<'_>>;

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
