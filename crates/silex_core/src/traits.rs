//! Lifetime-aware reactive traits.

use crate::{
    Callback, CallbackInvokeError, ErrorHandlerInput, NodeRef, Rx, RxInner, RxValueKind, Scope,
    SilexError, SilexResult,
    callback::map_callback_error,
    reactivity::{
        Memo, ReactiveSource, ReadSignal, RwSignal, Signal, SignalSlice, StoredValue, WriteSignal,
    },
};
use silex_reactivity::{ReactiveError, notify as raw_notify};
use std::fmt::Debug;

/// Values accepted by the scoped runtime.
pub trait RxData {}
impl<T: ?Sized> RxData for T {}

pub trait RxCloneData: Clone {}
impl<T: Clone> RxCloneData for T {}

pub trait RxError: Clone + Debug {}
impl<T: Clone + Debug> RxError for T {}

/// Construct a scope-owned reactive wrapper from an explicit value.
///
/// Unlike [`From`], this trait receives the [`Scope`] that owns the node.
/// Implementations must create every node, callback, and owner resource from
/// that scope. They must not create a [`crate::Runtime`], a detached scope, or use
/// thread-local runtime state.
pub trait RxFrom<'scope>: Sized {
    type Value: 'scope;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>;
}

/// Convert an existing scoped reactive source or an explicitly supported
/// value into a target reactive wrapper.
///
/// Unlike [`Into`], this trait receives the [`Scope`] that owns a node created
/// for a constant value. Existing sources are converted without materializing
/// another node, so their runtime provenance remains attached to the original
/// handle.
#[diagnostic::on_unimplemented(
    message = "reactive input must be an existing scoped source or an explicitly supported value",
    note = "constant reactive inputs require a Scope<'scope> and only the framework-supported value types are accepted"
)]
pub trait ReactiveInput<'scope, Target>: Sized {
    fn into_reactive_input(self, scope: Scope<'scope>) -> SilexResult<Target>;
}

/// Construct a scope-owned reactive wrapper from its value's default.
///
/// Every [`RxFrom`] implementation automatically implements this trait. The
/// default operation only delegates to [`RxFrom::rx_from`], so it cannot
/// create a [`crate::Runtime`], a detached scope, or thread-local runtime state.
pub trait RxDefault<'scope>: RxFrom<'scope> {
    fn rx_default(scope: Scope<'scope>) -> SilexResult<Self>
    where
        Self::Value: Default,
    {
        Self::rx_from(scope, Self::Value::default())
    }
}

impl<'scope, T> RxDefault<'scope> for T where T: RxFrom<'scope> {}

impl<'scope, T: 'scope> RxFrom<'scope> for Signal<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.stored(value.into()).map(Into::into)
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
    fn runtime_scope(&self) -> Scope<'_>;
}

impl<'scope, T, M> RuntimeScoped for Rx<'scope, T, M> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.scope
    }
}

impl<'scope, T> RuntimeScoped for ReadSignal<'scope, T> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.scope
    }
}

impl<'scope, T> RuntimeScoped for WriteSignal<'scope, T> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.scope
    }
}

impl<'scope, T> RuntimeScoped for RwSignal<'scope, T> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.read.runtime_scope()
    }
}

impl<'scope, T> RuntimeScoped for Memo<'scope, T> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.scope
    }
}

impl<'scope, T> RuntimeScoped for StoredValue<'scope, T> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.scope
    }
}

impl<'scope, T> RuntimeScoped for Signal<'scope, T> {
    fn runtime_scope(&self) -> Scope<'_> {
        self.rx.runtime_scope()
    }
}

impl<S, F, O: ?Sized> RuntimeScoped for SignalSlice<S, F, O>
where
    S: RuntimeScoped,
{
    fn runtime_scope(&self) -> Scope<'_> {
        self.source.runtime_scope()
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for ReadSignal<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.signal(value.into()).map(|(read, _)| read)
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for RwSignal<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.rw_signal(value.into())
    }
}

impl<'scope, T: Clone + PartialEq + 'scope> RxFrom<'scope> for Memo<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        let value = value.into();
        scope.memo_infallible(move |_| value.clone())
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for StoredValue<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.stored(value.into())
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for Rx<'scope, T, RxValueKind> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.constant(value.into())
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for Callback<'scope, T> {
    type Value = ();

    fn rx_from<V>(scope: Scope<'scope>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.callback(|_: T| Ok::<(), SilexError>(()))
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for NodeRef<'scope, T> {
    type Value = ();

    fn rx_from<V>(scope: Scope<'scope>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.node_ref()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for Signal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Signal<'scope, T>> {
        Ok(self)
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for ReadSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Signal<'scope, T>> {
        Ok(self.into())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Signal<'scope, T>> {
        Ok(self.into())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for Memo<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Signal<'scope, T>> {
        Ok(self.into())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for StoredValue<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Signal<'scope, T>> {
        Ok(self.into())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for Rx<'scope, T, RxValueKind> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Signal<'scope, T>> {
        Ok(self.into_signal())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, ReadSignal<'scope, T>> for ReadSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<ReadSignal<'scope, T>> {
        Ok(self)
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, ReadSignal<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<ReadSignal<'scope, T>> {
        Ok(self.read_signal())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, RwSignal<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<RwSignal<'scope, T>> {
        Ok(self)
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Memo<'scope, T>> for Memo<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Memo<'scope, T>> {
        Ok(self)
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, StoredValue<'scope, T>> for StoredValue<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<StoredValue<'scope, T>> {
        Ok(self)
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for Signal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Rx<'scope, T>> {
        Ok(self.into_rx())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for ReadSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Rx<'scope, T>> {
        Ok(self.into_rx())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Rx<'scope, T>> {
        Ok(self.into_rx())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for Memo<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Rx<'scope, T>> {
        Ok(self.into_rx())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for StoredValue<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Rx<'scope, T>> {
        Ok(self.into_rx())
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for Rx<'scope, T, RxValueKind> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> SilexResult<Rx<'scope, T>> {
        Ok(self)
    }
}

macro_rules! impl_reactive_input_values {
    ($($value:ty),* $(,)?) => {
        $(
            impl<'scope> ReactiveInput<'scope, Signal<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<Signal<'scope, $value>> {
                    <Signal<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, ReadSignal<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<ReadSignal<'scope, $value>> {
                    <ReadSignal<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, RwSignal<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<RwSignal<'scope, $value>> {
                    <RwSignal<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, Memo<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<Memo<'scope, $value>> {
                    <Memo<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, StoredValue<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<StoredValue<'scope, $value>> {
                    <StoredValue<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, Rx<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<Rx<'scope, $value>> {
                    <Rx<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
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
            impl<'scope, 'value> ReactiveInput<'scope, Signal<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<Signal<'scope, $target>> {
                    <Signal<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, ReadSignal<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<ReadSignal<'scope, $target>> {
                    <ReadSignal<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, RwSignal<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<RwSignal<'scope, $target>> {
                    <RwSignal<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, Memo<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<Memo<'scope, $target>> {
                    <Memo<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, StoredValue<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<StoredValue<'scope, $target>> {
                    <StoredValue<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, Rx<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> SilexResult<Rx<'scope, $target>> {
                    <Rx<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
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

impl<'scope, T: 'scope> RxValue for ReadSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxRead for ReadSignal<'scope, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(SilexError::fatal)
    }
}

impl<'scope, T: 'scope> RxValue for WriteSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxWrite for WriteSignal<'scope, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        raw_notify(&self.inner).map_err(SilexError::fatal)
    }
}

impl<'scope, T: 'scope> RxValue for RwSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxRead for RwSignal<'scope, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.read.with(f)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.read.with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxWrite for RwSignal<'scope, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.write.rx_update_untracked(f)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        self.write.rx_notify()
    }
}

impl<'scope, T: 'scope> RxValue for Signal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxRead for Signal<'scope, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.rx.with(f)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.rx.with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxValue for Rx<'scope, T, RxValueKind> {
    type Value = T;
}

impl<'scope, T: 'scope> RxRead for Rx<'scope, T, RxValueKind> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.with(f).map_err(SilexError::fatal),
            RxInner::Memo(memo) => memo.with(f).map_err(map_callback_error),
            RxInner::Derived(derived) => derived.with(f).map_err(|error| match error {
                CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
                CallbackInvokeError::User(error) => error,
                CallbackInvokeError::Handler(error) => {
                    SilexError::fatal(ReactiveError::Handler(error))
                }
            }),
            RxInner::Stored(stored) => stored.with(f).map_err(SilexError::fatal),
        }
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.with_untracked(f).map_err(SilexError::fatal),
            RxInner::Memo(memo) => memo.with_untracked(f).map_err(map_callback_error),
            RxInner::Derived(derived) => derived.with_untracked(f).map_err(|error| match error {
                CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
                CallbackInvokeError::User(error) => error,
                CallbackInvokeError::Handler(error) => {
                    SilexError::fatal(ReactiveError::Handler(error))
                }
            }),
            RxInner::Stored(stored) => stored.with(f).map_err(SilexError::fatal),
        }
    }
}

impl<'scope, T: 'scope> RxValue for StoredValue<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxRead for StoredValue<'scope, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }
}

impl<'scope, T: 'scope> RxWrite for StoredValue<'scope, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        Ok(())
    }
}

impl<'scope, T: 'scope> RxValue for Memo<'scope, T> {
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

impl<'scope, T: 'scope> RxRead for Memo<'scope, T> {
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
    fn map_or<'scope, U, H>(
        &self,
        scope: Scope<'scope>,
        default: U,
        f: impl Fn(&T) -> U + 'scope,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        U: Clone + 'scope,
        T: 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self.clone(), error_handler)?;
        scope.derived(
            move || source.with(|value| value.as_ref().map(&f).unwrap_or_else(|| default.clone())),
            error_handler,
        )
    }

    fn unwrap_or<'scope, H>(
        &self,
        scope: Scope<'scope>,
        default: T,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, T>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        T: PartialEq + Clone + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self.clone(), error_handler)?;
        scope
            .memo(
                move |_| {
                    source.with(|value| value.as_ref().cloned().unwrap_or_else(|| default.clone()))
                },
                error_handler,
            )
            .map(|memo| memo.into_rx())
    }

    fn map_or_else<'scope, U, H>(
        &self,
        scope: Scope<'scope>,
        default: impl Fn() -> U + 'scope,
        f: impl Fn(&T) -> U + 'scope,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        U: 'scope,
        T: 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self.clone(), error_handler)?;
        scope.derived(
            move || source.with(|value| value.as_ref().map(&f).unwrap_or_else(&default)),
            error_handler,
        )
    }

    fn and_then<'scope, U, H>(
        &self,
        scope: Scope<'scope>,
        f: impl Fn(&T) -> Option<U> + 'scope,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, Option<U>>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        U: 'scope,
        T: 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self.clone(), error_handler)?;
        scope.derived(
            move || source.with(|value| value.as_ref().and_then(&f)),
            error_handler,
        )
    }

    fn is_some_and<'scope, H>(
        &self,
        scope: Scope<'scope>,
        f: impl Fn(&T) -> bool + 'scope,
        error_handler: H,
    ) -> SilexResult<Rx<'scope, bool>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        T: 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let error_handler = error_handler.handler_ref();
        let source = scope.promote(self.clone(), error_handler)?;
        scope
            .memo(
                move |_| source.with(|value| value.as_ref().is_some_and(&f)),
                error_handler,
            )
            .map(|memo| memo.into_rx())
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
