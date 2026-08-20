pub mod callback;
pub mod context;
pub mod error;
pub mod log;
pub mod logic;
pub mod node_ref;
mod owner;
pub mod reactivity;
pub mod store;
mod task;
pub mod traits;

use std::marker::PhantomData;

use silex_reactivity::{
    Computed as RxComputed, ReadSignal as RxReadSignal, StoredValue as RxStoredValue,
};

pub use callback::Callback;
pub use context::{SilexContext, SilexContextProvider};
pub use error::{
    ErrorHandler, ErrorHandlerAnchor, ErrorHandlerInput, ErrorHandlerToken, ErrorReporter,
    ErrorSeverity, HandlerLease, SilexError, SilexErrorKind, SilexResult,
};

#[cfg(feature = "error-persistence")]
pub use error::{PersistenceError, PersistenceErrorKind};

#[cfg(feature = "error-i18n")]
pub use error::{I18nError, I18nErrorKind};

#[cfg(feature = "error-router")]
pub use error::{
    PathError, PathErrorKind, PathParamError, PathParamErrorKind, RoutePatternError,
    RoutePatternErrorKind,
};

#[cfg(feature = "error-net")]
pub use error::{NetConnectionState, NetError, NetErrorKind};

#[cfg(feature = "error-intl")]
pub use error::{IntlError, IntlErrorKind};

#[cfg(feature = "error-dom")]
pub use error::{
    CleanupFailure, CleanupFailureDiagnostic, CleanupOrigin, CleanupReport, CleanupSink,
    DisposeError, DropFailureReport, MountAvailability, MountError, RollbackError,
};

#[cfg(feature = "error-bootstrap")]
pub use error::{AppHostError, BootstrapError, HostState, UnmountOutcome};
pub use node_ref::NodeRef;
pub use owner::{OwnerAccess, OwnerChild, OwnerCleanupRegistrationError, OwnerHandle, Runtime};
pub use reactivity::{
    Computed, Constant, EffectHandle, Mutation, PromotionPlan, ReactiveSource, ReadSignal,
    Resource, ResourceBuilder, ResourceFetchBuilder, ResourceSource, ResourceSourceBuilder,
    RwSignal, Signal, StoredValue, SuspenseContext, WatchOptions, WriteSignal,
};
#[cfg(feature = "test-support")]
pub use silex_reactivity::RuntimeSnapshot;
pub use silex_reactivity::{CallbackInvokeError, CompletionSubmitError};
pub type CompletionOnce<T> = silex_reactivity::CompletionOnce<T, SilexError>;
pub type CompletionSender<T> = silex_reactivity::CompletionSender<T, SilexError>;
pub use silex_reactivity::unwind_safe;
pub use store::StoreField;
pub use task::TaskHandle;
pub use traits::{
    ReactiveInput, RuntimeScoped, RxData, RxDefault, RxFrom, RxGet, RxRead, RxValue, RxWrite,
};

pub use silex_reactivity::ReactiveError;
pub use silex_reactivity::{
    CleanupDiagnostic, CleanupPayloadKind, CloseError, CloseFailure, ClosePhase, CloseSource,
    CloseTransaction,
};

/// Marker for value-producing reactive nodes.
pub struct RxValueKind;

/// Marker retained for callback-oriented generic code.
pub struct RxEffectKind;

pub(crate) enum RxInner<'scope, T> {
    Signal(RxReadSignal<'scope, T>),
    Computed(RxComputed<'scope, T, SilexError>),
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
            (Self::Computed(a), Self::Computed(b)) => a == b,
            (Self::Stored(a), Self::Stored(b)) => a == b,
            _ => false,
        }
    }
}

impl<'scope, T> Eq for RxInner<'scope, T> {}

/// A typed reactive value that retains the scope used to create derived nodes.
pub struct Rx<'scope, T, M = RxValueKind> {
    pub(crate) inner: RxInner<'scope, T>,
    pub(crate) owner: OwnerAccess<'scope>,
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
        self.inner == other.inner && self.owner == other.owner
    }
}

impl<'scope, T, M> Eq for Rx<'scope, T, M> {}

impl<'scope, T: 'scope> Rx<'scope, T, RxValueKind> {
    pub(crate) fn from_signal(signal: ReadSignal<'scope, T>) -> Self {
        Self {
            inner: RxInner::Signal(signal.inner),
            owner: signal.owner,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_computed(computed: Computed<'scope, T>) -> Self {
        Self {
            inner: RxInner::Computed(computed.inner),
            owner: computed.owner,
            marker: PhantomData,
        }
    }

    pub(crate) fn from_stored(stored: StoredValue<'scope, T>) -> Self {
        Self {
            inner: RxInner::Stored(stored.inner),
            owner: stored.owner,
            marker: PhantomData,
        }
    }

    pub fn owner(&self) -> OwnerAccess<'scope> {
        self.owner
    }

    pub fn map<U, F, H>(self, f: F, error_handler: H) -> SilexResult<Rx<'scope, U>>
    where
        U: 'scope,
        F: Fn(&T) -> U + 'scope,
        H: ErrorHandlerInput<'scope>,
    {
        let owner = self.owner;
        owner
            .computed_always(move || self.with(|value| f(value)), error_handler)
            .map(Computed::into_rx)
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

/// Create a reactive value from an explicit component ctx.
///
/// The expansion returns a `SilexResult` and uses `?` internally to propagate
/// scope, promotion, and initial computation errors. The ctx supplies both
/// the scope and the [`ErrorHandler`] used for deferred errors.
///
/// `$source` is read through its existing tracked value access semantics. Use
/// `$(source.field)` when the field itself is the reactive source, such as a
/// field generated by `#[store]`.
#[macro_export]
macro_rules! rx {
    ($ctx:expr; $($body:tt)*) => {
        $crate::__internal_rx!($crate; @ctx $ctx; $($body)*)
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
        Callback, CompletionOnce, CompletionSender, ErrorHandler, ErrorHandlerInput,
        ErrorHandlerToken, ErrorReporter, NodeRef, OwnerAccess, OwnerHandle, ReactiveError,
        Runtime, Rx, SilexContext, SilexContextProvider, SilexError, SilexErrorKind, SilexResult,
        StoreField, batch_read, batch_read_untracked, logic::*, reactivity::*, rx, traits::*,
        unwind_safe,
    };
}
