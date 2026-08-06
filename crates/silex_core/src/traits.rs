//! Lifetime-aware reactive traits.

use crate::{
    Rx, RxInner, RxValueKind, Scope, SilexError, SilexResult,
    error::handle_error,
    reactivity::dispatch,
    reactivity::{Memo, ReactiveSource, ReadSignal, RwSignal, Signal, StoredValue, WriteSignal},
};
use std::{fmt::Debug, rc::Rc};

/// Values accepted by the scoped runtime.
pub trait RxData {}
impl<T: ?Sized> RxData for T {}

pub trait RxCloneData: Clone {}
impl<T: Clone> RxCloneData for T {}

pub trait RxError: Clone + Debug {}
impl<T: Clone + Debug> RxError for T {}

pub trait RxValue {
    type Value: ?Sized;
}

/// Common diagnostics and dependency tracking for a reactive value.
pub trait RxBase: RxValue {
    fn track(&self);

    fn debug_name(&self) -> Option<String> {
        None
    }
}

/// Closure-based tracked and untracked access. No reference can outlive the
/// callback supplied to these methods.
pub trait RxRead: RxBase {
    fn try_with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with(f).unwrap_or_else(|| panic_disposed(self))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    fn with_untracked<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with_untracked(f)
            .unwrap_or_else(|| panic_disposed(self))
    }
}

#[cold]
fn panic_disposed<T: RxBase + ?Sized>(value: &T) -> ! {
    dispatch::report_disposed(value.debug_name())
}

/// Clone-based convenience access built on top of [`RxRead`].
pub trait RxGet: RxRead
where
    Self::Value: Sized + Clone,
{
    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.try_with_untracked(Clone::clone)
    }

    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked()
            .unwrap_or_else(|| panic_disposed(self))
    }

    fn try_get(&self) -> Option<Self::Value> {
        self.try_with(Clone::clone)
    }

    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(|| panic_disposed(self))
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
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;

    fn rx_notify(&self);

    fn update(&self, f: impl FnOnce(&mut Self::Value)) {
        self.try_update(f).unwrap_or_else(|| panic_disposed(self));
    }

    fn try_update<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        let result = self.rx_try_update_untracked(f)?;
        self.rx_notify();
        Some(result)
    }

    fn set(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update(|current| *current = value);
    }

    fn try_update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        self.rx_try_update_untracked(f)
    }

    fn update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> U {
        self.try_update_untracked(f)
            .unwrap_or_else(|| panic_disposed(self))
    }

    fn set_untracked(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update_untracked(|current| *current = value);
    }

    fn notify(&self) {
        self.rx_notify();
    }

    fn setter(self, value: Self::Value) -> impl Fn() + Clone
    where
        Self: Sized + Clone,
        Self::Value: Sized + Clone,
    {
        move || self.set(value.clone())
    }

    fn updater<F>(self, f: F) -> impl Fn() + Clone
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

impl<'scope, T: 'scope> RxBase for ReadSignal<'scope, T> {
    fn track(&self) {
        self.inner.with(|_| ());
    }
}

impl<'scope, T: 'scope> RxRead for ReadSignal<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.inner.try_with(f).ok()
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.inner.with_untracked(f).ok()
    }
}

impl<'scope, T: 'scope> RxValue for WriteSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for WriteSignal<'scope, T> {
    fn track(&self) {}
}

impl<'scope, T: 'scope> RxWrite for WriteSignal<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        self.inner.try_update(f).ok()
    }

    fn rx_notify(&self) {}
}

impl<'scope, T: 'scope> RxValue for RwSignal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for RwSignal<'scope, T> {
    fn track(&self) {
        self.read.track();
    }
}

impl<'scope, T: 'scope> RxRead for RwSignal<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.read.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.read.try_with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxWrite for RwSignal<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        self.write.rx_try_update_untracked(f)
    }

    fn rx_notify(&self) {
        self.write.rx_notify();
    }
}

impl<'scope, T: 'scope> RxValue for Signal<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for Signal<'scope, T> {
    fn track(&self) {
        self.rx.track();
    }
}

impl<'scope, T: 'scope> RxRead for Signal<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.rx.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.rx.try_with_untracked(f)
    }
}

impl<'scope, T: 'scope> RxValue for Rx<'scope, T, RxValueKind> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for Rx<'scope, T, RxValueKind> {
    fn track(&self) {
        match &self.inner {
            RxInner::Signal(signal) => signal.with(|_| ()),
            RxInner::Memo(memo) => memo.with(|_| ()),
            RxInner::Derived(derived) => derived.with(|_| ()),
            RxInner::Stored(_) => {}
        }
    }
}

impl<'scope, T: 'scope> RxRead for Rx<'scope, T, RxValueKind> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        match &self.inner {
            RxInner::Signal(signal) => Some(signal.with(f)),
            RxInner::Memo(memo) => Some(memo.with(f)),
            RxInner::Derived(derived) => Some(derived.with(f)),
            RxInner::Stored(stored) => Some(stored.with(f)),
        }
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        match &self.inner {
            RxInner::Signal(signal) => signal.with_untracked(f).ok(),
            RxInner::Memo(memo) => memo.with_untracked(f).ok(),
            RxInner::Derived(derived) => derived.with_untracked(f).ok(),
            RxInner::Stored(stored) => Some(stored.with(f)),
        }
    }
}

impl<'scope, T: 'scope> RxValue for StoredValue<'scope, T> {
    type Value = T;
}

impl<'scope, T: 'scope> RxBase for StoredValue<'scope, T> {
    fn track(&self) {}
}

impl<'scope, T: 'scope> RxRead for StoredValue<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(self.inner.with(f))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(self.inner.with(f))
    }
}

impl<'scope, T: 'scope> RxWrite for StoredValue<'scope, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        Some(self.inner.update(f))
    }

    fn rx_notify(&self) {}
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
    fn track(&self) {
        self.inner.with(|_| ());
    }
}

impl<'scope, T: 'scope> RxRead for Memo<'scope, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(self.inner.with(f))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.inner.with_untracked(f).ok()
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

#[derive(Clone)]
pub struct ForErrorHandler(Rc<dyn Fn(SilexError)>);

impl ForErrorHandler {
    pub fn call(&self, error: SilexError) {
        (self.0)(error);
    }
}

impl<F> From<F> for ForErrorHandler
where
    F: Fn(SilexError) + 'static,
{
    fn from(value: F) -> Self {
        Self(Rc::new(value))
    }
}

impl Default for ForErrorHandler {
    fn default() -> Self {
        Self(Rc::new(handle_error))
    }
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
