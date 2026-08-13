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
pub use error::{ErrorHandler, ErrorReporter, SilexError, SilexErrorKind, SilexResult};
pub use node_ref::NodeRef;
pub use reactivity::{
    Constant, Effect, Memo, Mutation, PromotionPlan, ReactiveSource, ReadSignal, Resource,
    RwSignal, Signal, StoredValue, SuspenseContext, WatchOptions, WriteSignal,
};
pub use scope::{OwnedScope, RootHandle, Runtime, Scope};
pub use silex_reactivity::CallbackInvokeError;
#[cfg(feature = "test-support")]
pub use silex_reactivity::RuntimeSnapshot;
pub type CompletionOnce<T> = silex_reactivity::CompletionOnce<T, SilexError>;
pub type CompletionSender<T> = silex_reactivity::CompletionSender<T, SilexError>;
pub use silex_reactivity::unwind_safe;
pub use store::StoreField;
pub use task::TaskHandle;
pub use traits::{
    ReactiveInput, RuntimeScoped, RxData, RxDefault, RxFrom, RxGet, RxRead, RxValue, RxWrite,
};

pub use silex_reactivity::{CleanupDiagnostic, CleanupError, CleanupPayloadKind};
pub use silex_reactivity::{ReactiveError, ReactiveResult};

/// Marker for value-producing reactive nodes.
pub struct RxValueKind;

/// Marker retained for callback-oriented generic code.
pub struct RxEffectKind;

pub(crate) enum RxInner<'scope, T> {
    Signal(RxReadSignal<'scope, T>),
    Memo(RxMemo<'scope, T, SilexError>),
    Derived(Derived<'scope, T, SilexError>),
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

    pub(crate) fn from_derived(
        derived: Derived<'scope, T, SilexError>,
        scope: Scope<'scope>,
    ) -> Self {
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

    pub fn scope(&self) -> Scope<'scope> {
        self.scope
    }

    pub fn map<U, F>(
        self,
        f: F,
        error_handler: ErrorHandler<'scope, SilexError>,
    ) -> SilexResult<Rx<'scope, U>>
    where
        U: 'scope,
        F: Fn(&T) -> U + 'scope,
    {
        let scope = self.scope;
        scope.derived(move || self.with(|value| f(value)), error_handler)
    }

    pub fn get(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        RxGet::get(self)
    }

    pub fn get_untracked(&self) -> SilexResult<T>
    where
        T: Clone,
    {
        RxGet::get_untracked(self)
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        RxRead::with(self, f)
    }

    pub fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        RxRead::with_untracked(self, f)
    }

    pub fn into_signal(self) -> crate::reactivity::Signal<'scope, T> {
        crate::reactivity::Signal::from_rx(self)
    }

    pub(crate) fn is_constant(&self) -> bool {
        matches!(self.inner, RxInner::Stored(_))
    }
}

pub use silex_rx::rx as __internal_rx;

/// Create a reactive value with an explicit scope.
///
/// The expansion uses `?` internally to propagate scope, promotion, and
/// initial computation errors, so it must be used in a `Result` context. The
/// caller must provide the [`ErrorHandler`] used for deferred errors.
///
/// `$source` is read through its existing tracked value access semantics. Use
/// `$(source.field)` when the field itself is the reactive source, such as a
/// field generated by `#[store]`.
#[macro_export]
macro_rules! rx {
    ($scope:expr; $error_handler:expr; $($body:tt)*) => {
        $crate::__internal_rx!($crate; $scope; $error_handler; $($body)*)
    };
}

/// Read several values through nested closure-based accesses.
///
/// Each source is read through a tracked access, so this macro also preserves
/// dependencies when used inside a computation owned by a different lexical
/// scope. Use [`batch_read_untracked!`] when no dependency should be recorded.
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
        let __silex_batch_source = $source;
        __silex_batch_source.with(|$param: &$ty| $body)
    }};
    ([$source:expr, $($rest:expr),+] => [$param:ident : $ty:ty, $($params:ident : $tys:ty),+] => $body:expr) => {{
        use $crate::traits::RxRead;
        let __silex_batch_source = $source;
        __silex_batch_source.with(|$param: &$ty| {
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
        Callback, CompletionOnce, CompletionSender, ErrorHandler, ErrorReporter, NodeRef,
        ReactiveError, ReactiveResult, Runtime, Rx, Scope, SilexError, SilexErrorKind, SilexResult,
        StoreField, batch_read, batch_read_untracked, logic::*, reactivity::*, rx, traits::*,
        unwind_safe,
    };
}
