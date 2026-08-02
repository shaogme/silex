//! Lifetime-aware reactive traits.

use crate::{
    Rx, RxInner, RxValueKind, Scope, SilexError, SilexResult,
    error::handle_error,
    reactivity::dispatch,
    reactivity::{Memo, ReadSignal, RwSignal, Signal, StoredValue, WriteSignal},
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

    fn is_alive(&self) -> bool {
        true
    }

    fn is_disposed(&self) -> bool {
        !self.is_alive()
    }

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

/// Convert a value to a scoped reactive node.
pub trait IntoRx<'scope, 'run>: RxValue {
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + RxData;

    fn is_constant(&self) -> bool;
}

/// Convert a value to the high-level read-only signal wrapper.
pub trait IntoSignal<'scope, 'run>: RxValue {
    fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + RxData;
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

    fn update_untracked<U>(&self, f: impl FnOnce(&mut Self::Value) -> U) -> U {
        self.rx_try_update_untracked(f)
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

impl<'scope, 'run, T: 'scope> RxValue for ReadSignal<'scope, 'run, T> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for ReadSignal<'scope, 'run, T> {
    fn track(&self) {
        self.inner.with(|_| ());
    }

    fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> RxRead for ReadSignal<'scope, 'run, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.inner.try_with(f).ok()
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.inner.with_untracked(f).ok()
    }
}

impl<'scope, 'run, T: 'scope> RxValue for WriteSignal<'scope, 'run, T> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for WriteSignal<'scope, 'run, T> {
    fn track(&self) {}

    fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> RxWrite for WriteSignal<'scope, 'run, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        self.inner.try_update(f).ok()
    }

    fn rx_notify(&self) {}
}

impl<'scope, 'run, T: 'scope> RxValue for RwSignal<'scope, 'run, T> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for RwSignal<'scope, 'run, T> {
    fn track(&self) {
        self.read.track();
    }

    fn is_alive(&self) -> bool {
        self.read.is_alive() && self.write.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> RxRead for RwSignal<'scope, 'run, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.read.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.read.try_with_untracked(f)
    }
}

impl<'scope, 'run, T: 'scope> RxWrite for RwSignal<'scope, 'run, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        self.write.rx_try_update_untracked(f)
    }

    fn rx_notify(&self) {
        self.write.rx_notify();
    }
}

impl<'scope, 'run, T: 'scope> RxValue for Signal<'scope, 'run, T> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for Signal<'scope, 'run, T> {
    fn track(&self) {
        self.rx.track();
    }

    fn is_alive(&self) -> bool {
        self.rx.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> RxRead for Signal<'scope, 'run, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.rx.try_with(f)
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.rx.try_with_untracked(f)
    }
}

impl<'scope, 'run, T: 'scope> RxValue for Rx<'scope, 'run, T, RxValueKind> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for Rx<'scope, 'run, T, RxValueKind> {
    fn track(&self) {
        match &self.inner {
            RxInner::Signal(signal) => {
                signal.with(|_| ());
            }
            RxInner::Memo(memo) => {
                memo.with(|_| ());
            }
            RxInner::Derived(derived) => {
                derived.with(|_| ());
            }
            RxInner::Stored(_) => {}
        }
    }

    fn is_alive(&self) -> bool {
        match &self.inner {
            RxInner::Signal(signal) => signal.is_alive(),
            RxInner::Memo(memo) => memo.is_alive(),
            RxInner::Derived(derived) => derived.is_alive(),
            RxInner::Stored(stored) => stored.is_alive(),
        }
    }
}

impl<'scope, 'run, T: 'scope> RxRead for Rx<'scope, 'run, T, RxValueKind> {
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

impl<'scope, 'run, T: 'scope> RxValue for StoredValue<'scope, 'run, T> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for StoredValue<'scope, 'run, T> {
    fn track(&self) {}

    fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> RxRead for StoredValue<'scope, 'run, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(self.inner.with(f))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(self.inner.with(f))
    }
}

impl<'scope, 'run, T: 'scope> RxWrite for StoredValue<'scope, 'run, T> {
    fn rx_try_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> Option<U> {
        Some(self.inner.update(f))
    }

    fn rx_notify(&self) {}
}

impl<'scope, 'run, T: 'scope> RxValue for Memo<'scope, 'run, T> {
    type Value = T;
}

impl<'scope, 'run, T: 'scope> RxBase for Memo<'scope, 'run, T> {
    fn track(&self) {
        self.inner.with(|_| ());
    }

    fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}

impl<'scope, 'run, T: 'scope> RxRead for Memo<'scope, 'run, T> {
    fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        Some(self.inner.with(f))
    }

    fn try_with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        self.inner.with_untracked(f).ok()
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for ReadSignal<'scope, 'run, T> {
    fn into_rx(self, _scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Rx::from_signal(self)
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for ReadSignal<'scope, 'run, T> {
    fn into_signal(self, _scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Signal::from(self)
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for RwSignal<'scope, 'run, T> {
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        self.read.into_rx(scope)
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for RwSignal<'scope, 'run, T> {
    fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        self.read.into_signal(scope)
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for Signal<'scope, 'run, T> {
    fn into_rx(self, _scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        self.rx
    }

    fn is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for Signal<'scope, 'run, T> {
    fn into_signal(self, _scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        self
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for Memo<'scope, 'run, T> {
    fn into_rx(self, _scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Rx::from_memo(self)
    }

    fn is_constant(&self) -> bool {
        false
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for Memo<'scope, 'run, T> {
    fn into_signal(self, _scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Signal::from(self)
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for StoredValue<'scope, 'run, T> {
    fn into_rx(self, _scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Rx::from_stored(self)
    }

    fn is_constant(&self) -> bool {
        true
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for StoredValue<'scope, 'run, T> {
    fn into_signal(self, _scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Signal::from(self)
    }
}

impl<'scope, 'run, T: 'scope> IntoRx<'scope, 'run> for Rx<'scope, 'run, T, RxValueKind> {
    fn into_rx(self, _scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        self
    }

    fn is_constant(&self) -> bool {
        self.is_constant()
    }
}

impl<'scope, 'run, T: 'scope> IntoSignal<'scope, 'run> for Rx<'scope, 'run, T, RxValueKind> {
    fn into_signal(self, _scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, T>
    where
        T: Sized + RxData,
    {
        Signal::from_rx(self)
    }
}

macro_rules! impl_primitive_into_rx {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RxValue for $ty {
                type Value = Self;
            }

            impl<'scope, 'run> IntoRx<'scope, 'run> for $ty {
                fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, Self> {
                    scope.constant(self)
                }

                fn is_constant(&self) -> bool { true }
            }

            impl<'scope, 'run> IntoSignal<'scope, 'run> for $ty {
                fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, Self> {
                    scope.constant(self).into_signal(scope)
                }
            }
        )*
    };
}

impl_primitive_into_rx!(
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

impl<'scope, 'run> IntoRx<'scope, 'run> for &str {
    fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, String> {
        scope.constant(self.to_owned())
    }

    fn is_constant(&self) -> bool {
        true
    }
}

impl<'scope, 'run> IntoSignal<'scope, 'run> for &str {
    fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, String> {
        scope.constant(self.to_owned()).into_signal(scope)
    }
}

macro_rules! impl_tuple_into_rx {
    ($($name:ident : $index:tt),+ $(,)?) => {
        impl<$($name),+> RxValue for ($($name,)+)
        where
            $($name: RxValue, $name::Value: Sized + RxData),+
        {
            type Value = ($($name::Value,)+);
        }

        impl<'scope, 'run, $($name),+> IntoRx<'scope, 'run> for ($($name,)+)
        where
            $($name: IntoRx<'scope, 'run> + RxRead + 'scope, $name::Value: Sized + RxData + Clone + 'run),+
        {
            fn into_rx(self, scope: &Scope<'scope, 'run>) -> Rx<'scope, 'run, Self::Value> {
                let scope = *scope;
                scope.derived(move || {
                    (
                        $(crate::traits::RxGet::get(&self.$index),)+
                    )
                })
            }

            fn is_constant(&self) -> bool {
                $(self.$index.is_constant() &&)+ true
            }
        }

        impl<'scope, 'run, $($name),+> IntoSignal<'scope, 'run> for ($($name,)+)
        where
            $($name: IntoRx<'scope, 'run> + RxRead + 'scope, $name::Value: Sized + RxData + Clone + 'run),+
        {
            fn into_signal(self, scope: &Scope<'scope, 'run>) -> Signal<'scope, 'run, Self::Value> {
                self.into_rx(scope).into_signal(scope)
            }
        }
    };
}

impl_tuple_into_rx!(A: 0);
impl_tuple_into_rx!(A: 0, B: 1);
impl_tuple_into_rx!(A: 0, B: 1, C: 2);
impl_tuple_into_rx!(A: 0, B: 1, C: 2, D: 3);
impl_tuple_into_rx!(A: 0, B: 1, C: 2, D: 3, E: 4);
impl_tuple_into_rx!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

/// Reactive helpers for `Option<T>` values.
pub trait RxOptionExt<T>: RxRead<Value = Option<T>> + Clone {
    fn map_or<'scope, 'run, U>(
        &self,
        scope: &Scope<'scope, 'run>,
        default: U,
        f: impl Fn(&T) -> U + 'scope,
    ) -> Memo<'scope, 'run, U>
    where
        Self: 'scope,
        U: PartialEq + Clone + 'run,
        T: 'scope,
    {
        let source = self.clone();
        scope.memo(move |_| {
            source.with(|value| value.as_ref().map(&f).unwrap_or_else(|| default.clone()))
        })
    }

    fn unwrap_or<'scope, 'run>(
        &self,
        scope: &Scope<'scope, 'run>,
        default: T,
    ) -> Memo<'scope, 'run, T>
    where
        Self: 'scope,
        T: PartialEq + Clone + 'run,
    {
        self.map_or(scope, default, Clone::clone)
    }

    fn map_or_else<'scope, 'run, U>(
        &self,
        scope: &Scope<'scope, 'run>,
        default: impl Fn() -> U + 'scope,
        f: impl Fn(&T) -> U + 'scope,
    ) -> Memo<'scope, 'run, U>
    where
        Self: 'scope,
        U: PartialEq + Clone + 'run,
        T: 'scope,
    {
        let source = self.clone();
        scope.memo(move |_| source.with(|value| value.as_ref().map(&f).unwrap_or_else(&default)))
    }

    fn and_then<'scope, 'run, U>(
        &self,
        scope: &Scope<'scope, 'run>,
        f: impl Fn(&T) -> Option<U> + 'scope,
    ) -> Memo<'scope, 'run, Option<U>>
    where
        Self: 'scope,
        U: PartialEq + Clone + 'run,
        T: 'scope,
    {
        let source = self.clone();
        scope.memo(move |_| source.with(|value| value.as_ref().and_then(&f)))
    }

    fn is_some_and<'scope, 'run>(
        &self,
        scope: &Scope<'scope, 'run>,
        f: impl Fn(&T) -> bool + 'scope,
    ) -> Memo<'scope, 'run, bool>
    where
        Self: 'scope,
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
