//! Lifetime-aware reactive traits.

use crate::{
    Callback, NodeRef, ReactiveError, ReactiveResult, Rx, RxInner, RxValueKind, Scope, SilexError,
    SilexResult,
    reactivity::{Memo, ReactiveSource, ReadSignal, RwSignal, Signal, StoredValue, WriteSignal},
};
use silex_reactivity::try_notify as raw_try_notify;
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

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>;
}

/// Fallible construction of a scope-owned reactive wrapper.
///
/// This trait is used by wrappers whose node registration can fail and whose
/// error must remain visible to the caller. Infallible value conversions keep
/// using [`RxFrom`].
pub trait TryRxFrom<'scope>: Sized {
    type Value: 'scope;

    fn try_rx_from<V>(scope: Scope<'scope>, value: V) -> SilexResult<Self>
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
    fn into_reactive_input(self, scope: Scope<'scope>) -> Target;
}

/// Construct a scope-owned reactive wrapper from its value's default.
///
/// Every [`RxFrom`] implementation automatically implements this trait. The
/// default operation only delegates to [`RxFrom::rx_from`], so it cannot
/// create a [`crate::Runtime`], a detached scope, or thread-local runtime state.
pub trait RxDefault<'scope>: RxFrom<'scope> {
    fn rx_default(scope: Scope<'scope>) -> Self
    where
        Self::Value: Default,
    {
        Self::rx_from(scope, Self::Value::default())
    }
}

impl<'scope, T> RxDefault<'scope> for T where T: RxFrom<'scope> {}

/// Fallible default construction for wrappers implementing [`TryRxFrom`].
pub trait TryRxDefault<'scope>: TryRxFrom<'scope> {
    fn try_rx_default(scope: Scope<'scope>) -> SilexResult<Self>
    where
        Self::Value: Default,
    {
        Self::try_rx_from(scope, Self::Value::default())
    }
}

impl<'scope, T> TryRxDefault<'scope> for T where T: TryRxFrom<'scope> {}

impl<'scope, T: 'scope> RxFrom<'scope> for Signal<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        scope.stored(value.into()).into()
    }
}

pub trait RxValue {
    type Value: ?Sized;
}

/// Common diagnostics and dependency tracking for a reactive value.
pub trait RxBase: RxValue {
    fn try_track(&self) -> ReactiveResult<()>;

    fn track(&self) {
        self.try_track().unwrap_or_else(panic_reactive);
    }

    fn debug_name(&self) -> Option<String> {
        None
    }
}

/// Closure-based tracked and untracked access. No reference can outlive the
/// callback supplied to these methods.
pub trait RxRead: RxBase {
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> ReactiveResult<U>;

    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with(f).unwrap_or_else(panic_reactive)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> ReactiveResult<U>;

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with_untracked(f).unwrap_or_else(panic_reactive)
    }
}

#[cold]
fn panic_reactive<T>(error: ReactiveError) -> T {
    panic!("reactive operation failed: {error}")
}

impl<'scope, T: 'scope> RxFrom<'scope> for ReadSignal<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        scope.signal(value.into()).0
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for RwSignal<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        scope.rw_signal(value.into())
    }
}

impl<'scope, T: Clone + PartialEq + 'scope> RxFrom<'scope> for Memo<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        let value = value.into();
        scope.memo(move |_| value.clone())
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for StoredValue<'scope, T> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        scope.stored(value.into())
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for Rx<'scope, T, RxValueKind> {
    type Value = T;

    fn rx_from<V>(scope: Scope<'scope>, value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        scope.constant(value.into())
    }
}

impl<'scope, T: 'scope> TryRxFrom<'scope> for Callback<'scope, T> {
    type Value = ();

    fn try_rx_from<V>(scope: Scope<'scope>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Value>,
    {
        scope.callback(|_: T| Ok::<(), SilexError>(()))
    }
}

impl<'scope, T: 'scope> RxFrom<'scope> for NodeRef<'scope, T> {
    type Value = ();

    fn rx_from<V>(scope: Scope<'scope>, _value: V) -> Self
    where
        V: Into<Self::Value>,
    {
        scope.node_ref()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for Signal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Signal<'scope, T> {
        self
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for ReadSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Signal<'scope, T> {
        self.into()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Signal<'scope, T> {
        self.into()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for Memo<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Signal<'scope, T> {
        self.into()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for StoredValue<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Signal<'scope, T> {
        self.into()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Signal<'scope, T>> for Rx<'scope, T, RxValueKind> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Signal<'scope, T> {
        self.into_signal()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, ReadSignal<'scope, T>> for ReadSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> ReadSignal<'scope, T> {
        self
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, ReadSignal<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> ReadSignal<'scope, T> {
        self.read_signal()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, RwSignal<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> RwSignal<'scope, T> {
        self
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Memo<'scope, T>> for Memo<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Memo<'scope, T> {
        self
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, StoredValue<'scope, T>> for StoredValue<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> StoredValue<'scope, T> {
        self
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for Signal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Rx<'scope, T> {
        self.into_rx()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for ReadSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Rx<'scope, T> {
        self.into_rx()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for RwSignal<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Rx<'scope, T> {
        self.into_rx()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for Memo<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Rx<'scope, T> {
        self.into_rx()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for StoredValue<'scope, T> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Rx<'scope, T> {
        self.into_rx()
    }
}

impl<'scope, T: 'scope> ReactiveInput<'scope, Rx<'scope, T>> for Rx<'scope, T, RxValueKind> {
    fn into_reactive_input(self, _scope: Scope<'scope>) -> Rx<'scope, T> {
        self
    }
}

macro_rules! impl_reactive_input_values {
    ($($value:ty),* $(,)?) => {
        $(
            impl<'scope> ReactiveInput<'scope, Signal<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> Signal<'scope, $value> {
                    <Signal<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, ReadSignal<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> ReadSignal<'scope, $value> {
                    <ReadSignal<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, RwSignal<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> RwSignal<'scope, $value> {
                    <RwSignal<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, Memo<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> Memo<'scope, $value> {
                    <Memo<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, StoredValue<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> StoredValue<'scope, $value> {
                    <StoredValue<'scope, $value> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope> ReactiveInput<'scope, Rx<'scope, $value>> for $value {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> Rx<'scope, $value> {
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
                ) -> Signal<'scope, $target> {
                    <Signal<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, ReadSignal<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> ReadSignal<'scope, $target> {
                    <ReadSignal<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, RwSignal<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> RwSignal<'scope, $target> {
                    <RwSignal<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, Memo<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> Memo<'scope, $target> {
                    <Memo<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, StoredValue<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> StoredValue<'scope, $target> {
                    <StoredValue<'scope, $target> as RxFrom<'scope>>::rx_from(scope, self)
                }
            }

            impl<'scope, 'value> ReactiveInput<'scope, Rx<'scope, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    scope: Scope<'scope>,
                ) -> Rx<'scope, $target> {
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
    fn try_get_untracked(&self) -> ReactiveResult<Self::Value> {
        self.try_with_untracked(Clone::clone)
    }

    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked().unwrap_or_else(panic_reactive)
    }

    fn try_get(&self) -> ReactiveResult<Self::Value> {
        self.try_with(Clone::clone)
    }

    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(panic_reactive)
    }
}

impl<T> RxGet for T
where
    T: RxRead + ?Sized,
    T::Value: Sized + Clone,
{
}

/// Unified scoped writes.
pub trait RxWrite: RxBase {
    fn rx_try_update_untracked<U>(
        &self,
        f: impl FnOnce(&mut Self::Value) -> U,
    ) -> ReactiveResult<U>;

    fn rx_try_notify(&self) -> ReactiveResult<()>;

    fn update(&self, f: impl FnOnce(&mut Self::Value)) {
        self.try_update(f).unwrap_or_else(panic_reactive);
    }

    fn try_update<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> ReactiveResult<U> {
        self.rx_try_update_untracked(f)
    }

    fn set(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update(|current| *current = value);
    }

    fn try_set(&self, value: Self::Value) -> ReactiveResult<()>
    where
        Self::Value: Sized,
    {
        self.try_update(|current| *current = value)
    }

    fn try_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> ReactiveResult<U> {
        self.rx_try_update_untracked(f)
    }

    fn update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> U {
        self.try_update_untracked(f).unwrap_or_else(panic_reactive)
    }

    fn set_untracked(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update_untracked(|current| *current = value);
    }

    fn try_notify(&self) -> ReactiveResult<()> {
        self.rx_try_notify()
    }

    fn notify(&self) {
        self.try_notify().unwrap_or_else(panic_reactive);
    }

    fn setter(self, value: Self::Value) -> impl Fn() -> SilexResult<()> + Clone
    where
        Self: Sized + Clone,
        Self::Value: Sized + Clone,
    {
        move || self.try_set(value.clone()).map_err(Into::into)
    }

    fn updater<F>(self, f: F) -> impl Fn() -> SilexResult<()> + Clone
    where
        Self: Sized + Clone,
        Self::Value: Sized,
        F: Fn(&mut Self::Value) + Clone,
    {
        move || self.try_update(f.clone()).map_err(Into::into)
    }
}

impl<'scope, T: 'scope> RxValue for ReadSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for ReadSignal<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        self.inner.try_with(|_| ()).map(|_| ())
    }
}

impl<'scope, T: 'scope> RxRead for ReadSignal<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxValue for WriteSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for WriteSignal<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        Ok(())
    }
}

impl<'scope, T: 'scope> RxWrite for WriteSignal<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> ReactiveResult<U> {
        self.inner.try_update(f)
    }

    fn rx_try_notify(&self) -> ReactiveResult<()> {
        raw_try_notify(&self.inner)
    }
}

impl<'scope, T: 'scope> RxValue for RwSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for RwSignal<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        self.read.try_track()
    }
}

impl<'scope, T: 'scope> RxRead for RwSignal<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.read.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.read.try_with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxWrite for RwSignal<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> ReactiveResult<U> {
        self.write.rx_try_update_untracked(f)
    }

    fn rx_try_notify(&self) -> ReactiveResult<()> {
        self.write.rx_try_notify()
    }
}

impl<'scope, T: 'scope> RxValue for Signal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for Signal<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        self.rx.try_track()
    }
}

impl<'scope, T: 'scope> RxRead for Signal<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.rx.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.rx.try_with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxValue for Rx<'scope, T, RxValueKind> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for Rx<'scope, T, RxValueKind> {
    fn try_track(&self) -> ReactiveResult<()> {
        match &self.inner {
            RxInner::Signal(signal) => signal.try_with(|_| ()).map(|_| ()),
            RxInner::Memo(memo) => memo.try_with(|_| ()).map(|_| ()),
            RxInner::Derived(derived) => derived.try_with(|_| ()).map(|_| ()),
            RxInner::Stored(stored) => stored.try_with(|_| ()).map(|_| ()),
        }
    }
}

impl<'scope, T: 'scope> RxRead for Rx<'scope, T, RxValueKind> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.try_with(f),
            RxInner::Memo(memo) => memo.try_with(f),
            RxInner::Derived(derived) => derived.try_with(f),
            RxInner::Stored(stored) => stored.try_with(f),
        }
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.try_with_untracked(f),
            RxInner::Memo(memo) => memo.try_with_untracked(f),
            RxInner::Derived(derived) => derived.try_with_untracked(f),
            RxInner::Stored(stored) => stored.try_with(f),
        }
    }
}

impl<'scope, T: 'scope> RxValue for StoredValue<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for StoredValue<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        Ok(())
    }
}

impl<'scope, T: 'scope> RxRead for StoredValue<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with(f)
    }
}

impl<'scope, T: 'scope> RxWrite for StoredValue<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> ReactiveResult<U> {
        self.inner.try_update(f)
    }

    fn rx_try_notify(&self) -> ReactiveResult<()> {
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

impl<'scope, T: 'scope> RxBase for Memo<'scope, T> {
    fn try_track(&self) -> ReactiveResult<()> {
        self.inner.try_with(|_| ()).map(|_| ())
    }
}

impl<'scope, T: 'scope> RxRead for Memo<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with_untracked(f)
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

/// Reactive helpers for `Option<T>` values.
pub trait RxOptionExt<T>: RxRead<Value = Option<T>> + Clone {
    fn map_or<'scope, U>(
        &self,
        scope: Scope<'scope>,
        default: U,
        f: impl Fn(&T) -> U + 'scope,
    ) -> Memo<'scope, U>
    where
        Self: ReactiveSource<'scope> + 'scope,
        U: PartialEq + Clone + 'scope,
        T: 'scope,
    {
        let source = scope.promote(self.clone());
        scope.memo_from(source.runtime_inputs(), move |_| {
            source.with(|value| value.as_ref().map(&f).unwrap_or_else(|| default.clone()))
        })
    }

    fn unwrap_or<'scope>(&self, scope: Scope<'scope>, default: T) -> Memo<'scope, T>
    where
        Self: ReactiveSource<'scope> + 'scope,
        T: PartialEq + Clone + 'scope,
    {
        self.map_or(scope, default, Clone::clone)
    }

    fn map_or_else<'scope, U>(
        &self,
        scope: Scope<'scope>,
        default: impl Fn() -> U + 'scope,
        f: impl Fn(&T) -> U + 'scope,
    ) -> Memo<'scope, U>
    where
        Self: ReactiveSource<'scope> + 'scope,
        U: PartialEq + Clone + 'scope,
        T: 'scope,
    {
        let source = scope.promote(self.clone());
        scope.memo_from(source.runtime_inputs(), move |_| {
            source.with(|value| value.as_ref().map(&f).unwrap_or_else(&default))
        })
    }

    fn and_then<'scope, U>(
        &self,
        scope: Scope<'scope>,
        f: impl Fn(&T) -> Option<U> + 'scope,
    ) -> Memo<'scope, Option<U>>
    where
        Self: ReactiveSource<'scope> + 'scope,
        U: PartialEq + Clone + 'scope,
        T: 'scope,
    {
        let source = scope.promote(self.clone());
        scope.memo_from(source.runtime_inputs(), move |_| {
            source.with(|value| value.as_ref().and_then(&f))
        })
    }

    fn is_some_and<'scope>(
        &self,
        scope: Scope<'scope>,
        f: impl Fn(&T) -> bool + 'scope,
    ) -> Memo<'scope, bool>
    where
        Self: ReactiveSource<'scope> + 'scope,
        T: 'scope,
    {
        self.map_or(scope, false, f)
    }

    fn if_some_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.with_untracked(|value| value.as_ref().map(f))
    }
}

impl<S, T> RxOptionExt<T> for S where S: RxRead<Value = Option<T>> + Clone {}

/// Sources used by list rendering helpers.
pub trait ForLoopSource {
    type Item: Clone;
    fn as_slice(&self) -> SilexResult<&[Self::Item]>;
}

impl<T: Clone + 'static> ForLoopSource for Vec<T> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self)
    }
}

impl<T: Clone + 'static> ForLoopSource for Option<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self.as_deref().unwrap_or_default())
    }
}
