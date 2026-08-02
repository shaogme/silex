pub mod callback;
pub mod error;
pub mod log;
pub mod logic;
pub mod macros_helper;
pub mod node_ref;
pub mod reactivity;
pub mod scope;
pub mod store;
pub mod traits;

use std::marker::PhantomData;

use silex_reactivity::{
    Derived, Memo as RxMemo, ReadSignal as RxReadSignal, StoredValue as RxStoredValue,
};

pub use callback::Callback;
pub use error::{ErrorContext, SilexError, SilexResult};
pub use node_ref::NodeRef;
pub use reactivity::{
    Constant, Effect, Memo, ReadSignal, RwSignal, Signal, StoredValue, WriteSignal,
};
pub use scope::{Runtime, Scope};
pub use store::Store;
pub use traits::{RxBase, RxGet, RxRead};

/// Marker for value-producing reactive nodes.
pub struct RxValueKind;

/// Marker retained for callback-oriented generic code.
pub struct RxEffectKind;

pub(crate) enum RxInner<'scope, 'run, T> {
    Signal(RxReadSignal<'scope, 'run, T>),
    Memo(RxMemo<'scope, 'run, T>),
    Derived(Derived<'scope, 'run, T>),
    Stored(RxStoredValue<'scope, 'run, T>),
}

impl<'scope, 'run, T> Copy for RxInner<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for RxInner<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A typed reactive value that retains the scope used to create derived nodes.
pub struct Rx<'scope, 'run, T, M = RxValueKind> {
    pub(crate) inner: RxInner<'scope, 'run, T>,
    pub(crate) scope: Scope<'scope, 'run>,
    pub(crate) marker: PhantomData<M>,
}

impl<'scope, 'run, T, M> Copy for Rx<'scope, 'run, T, M> {}

impl<'scope, 'run, T, M> Clone for Rx<'scope, 'run, T, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, 'run, T: 'scope> Rx<'scope, 'run, T, RxValueKind> {
    pub(crate) fn from_signal(signal: ReadSignal<'scope, 'run, T>) -> Self {
        Self {
            inner: RxInner::Signal(signal.inner),
            scope: signal.scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_memo(memo: Memo<'scope, 'run, T>) -> Self {
        Self {
            inner: RxInner::Memo(memo.inner),
            scope: memo.scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_derived(
        derived: Derived<'scope, 'run, T>,
        scope: Scope<'scope, 'run>,
    ) -> Self {
        Self {
            inner: RxInner::Derived(derived),
            scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_stored(stored: StoredValue<'scope, 'run, T>) -> Self {
        Self {
            inner: RxInner::Stored(stored.inner),
            scope: stored.scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn scope(&self) -> Scope<'scope, 'run> {
        self.scope
    }

    pub fn map<U, F>(self, f: F) -> Rx<'scope, 'run, U>
    where
        U: 'run,
        F: Fn(&T) -> U + 'scope,
    {
        let scope = self.scope;
        scope.derived(move || self.with(|value| f(value)))
    }

    pub fn get(&self) -> T
    where
        T: Clone,
    {
        RxGet::get(self)
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        RxRead::with(self, f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        RxRead::with_untracked(self, f)
    }

    pub fn is_alive(&self) -> bool {
        RxBase::is_alive(self)
    }

    pub(crate) fn is_constant(&self) -> bool {
        matches!(self.inner, RxInner::Stored(_))
    }
}

pub use silex_rx::rx as __internal_rx;

/// Create a reactive value with an explicit scope.
#[macro_export]
macro_rules! rx {
    ($scope:expr; $($body:tt)*) => {
        $crate::__internal_rx!($crate; $scope; $($body)*)
    };
}

/// Read several values through nested closure-based accesses.
#[macro_export]
macro_rules! batch_read {
    ($($source:expr),+ => |$($param:ident : $ty:ty),+| $body:expr) => {
        $crate::batch_read_recurse!([$($source),+] => [$($param : $ty),+] => $body)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! batch_read_recurse {
    ([$source:expr] => [$param:ident : $ty:ty] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($source).with(|$param: &$ty| $body)
    }};
    ([$source:expr, $($rest:expr),+] => [$param:ident : $ty:ty, $($params:ident : $tys:ty),+] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($source).with(|$param: &$ty| {
            $crate::batch_read_recurse!([$($rest),+] => [$($params : $tys),+] => $body)
        })
    }};
}

#[macro_export]
macro_rules! batch_read_untracked {
    ($($source:expr),+ => |$($param:ident : $ty:ty),+| $body:expr) => {
        $crate::batch_read_untracked_recurse!([$($source),+] => [$($param : $ty),+] => $body)
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! batch_read_untracked_recurse {
    ([$source:expr] => [$param:ident : $ty:ty] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($source).with_untracked(|$param: &$ty| $body)
    }};
    ([$source:expr, $($rest:expr),+] => [$param:ident : $ty:ty, $($params:ident : $tys:ty),+] => $body:expr) => {{
        use $crate::traits::RxRead;
        ($source).with_untracked(|$param: &$ty| {
            $crate::batch_read_untracked_recurse!([$($rest),+] => [$($params : $tys),+] => $body)
        })
    }};
}

pub mod prelude {
    pub use crate::{
        Callback, ErrorContext, NodeRef, Runtime, Rx, Scope, SilexError, SilexResult, Store,
        batch_read, batch_read_untracked, logic::*, reactivity::*, rx, traits::*,
    };
}
