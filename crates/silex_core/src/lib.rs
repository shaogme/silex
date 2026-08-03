pub mod callback;
pub mod error;
pub mod log;
pub mod logic;
pub mod macros_helper;
pub mod node_ref;
pub mod reactivity;
pub mod scope;
pub mod store;
mod task;
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
pub use scope::{OwnedScope, RootHandle, RootScope, Runtime, Scope};
pub use silex_reactivity::CompletionToken;
#[doc(hidden)]
pub use silex_reactivity::RuntimeInputs;
pub use store::Store;
pub use task::TaskHandle;
pub use traits::{RxBase, RxGet, RxRead};

pub use silex_reactivity::{
    CleanupError, RootCallback, RootDerived, RootEffect, RootMemo, RootNodeRef, RootReadSignal,
    RootSignal, RootStoredValue, RootWriteSignal,
};

/// Marker for value-producing reactive nodes.
pub struct RxValueKind;

/// Marker retained for callback-oriented generic code.
pub struct RxEffectKind;

pub(crate) enum RxInner<'scope, T> {
    Signal(RxReadSignal<'scope, T>),
    Memo(RxMemo<'scope, T>),
    Derived(Derived<'scope, T>),
    Stored(RxStoredValue<'scope, T>),
}

impl<'scope, T> Copy for RxInner<'scope, T> {}

impl<'scope, T> Clone for RxInner<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> PartialEq for RxInner<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Signal(a), Self::Signal(b)) => a == b,
            (Self::Memo(a), Self::Memo(b)) => a == b,
            (Self::Derived(a), Self::Derived(b)) => a == b,
            (Self::Stored(a), Self::Stored(b)) => a == b,
            _ => false,
        }
    }
}

impl<'scope, T> Eq for RxInner<'scope, T> {}

/// A typed reactive value that retains the scope used to create derived nodes.
pub struct Rx<'scope, T, M = RxValueKind> {
    pub(crate) inner: RxInner<'scope, T>,
    pub(crate) scope: Scope<'scope>,
    pub(crate) marker: PhantomData<M>,
}

impl<'scope, T, M> Copy for Rx<'scope, T, M> {}

impl<'scope, T, M> Clone for Rx<'scope, T, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T, M> PartialEq for Rx<'scope, T, M> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.scope == other.scope
    }
}

impl<'scope, T, M> Eq for Rx<'scope, T, M> {}

impl<'scope, T: 'scope> Rx<'scope, T, RxValueKind> {
    pub(crate) fn from_signal(signal: ReadSignal<'scope, T>) -> Self {
        Self {
            inner: RxInner::Signal(signal.inner),
            scope: signal.scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_memo(memo: Memo<'scope, T>) -> Self {
        Self {
            inner: RxInner::Memo(memo.inner),
            scope: memo.scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_derived(derived: Derived<'scope, T>, scope: Scope<'scope>) -> Self {
        Self {
            inner: RxInner::Derived(derived),
            scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_stored(stored: StoredValue<'scope, T>) -> Self {
        Self {
            inner: RxInner::Stored(stored.inner),
            scope: stored.scope,
            marker: PhantomData,
        }
    }

    pub(crate) fn scope(&self) -> Scope<'scope> {
        self.scope
    }

    pub fn map<U, F>(self, f: F) -> Rx<'scope, U>
    where
        U: 'scope,
        F: Fn(&T) -> U + 'scope,
    {
        let scope = self.scope;
        scope.derived_from(self.runtime_inputs(), move || self.with(|value| f(value)))
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

    pub fn into_signal(self) -> crate::reactivity::Signal<'scope, T> {
        crate::reactivity::Signal::from_rx(self)
    }

    #[doc(hidden)]
    pub fn runtime_inputs(&self) -> RuntimeInputs {
        let input = match &self.inner {
            RxInner::Signal(signal) => signal.runtime_input(),
            RxInner::Memo(memo) => memo.runtime_input(),
            RxInner::Derived(derived) => derived.runtime_input(),
            RxInner::Stored(stored) => stored.runtime_input(),
        };
        RuntimeInputs::single(input)
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
        Callback, CompletionToken, ErrorContext, NodeRef, Runtime, Rx, Scope, SilexError,
        SilexResult, Store, batch_read, batch_read_untracked, logic::*, reactivity::*, rx,
        traits::*,
    };
}
