use std::fmt;

#[cfg(any(feature = "error-dom", feature = "error-bootstrap"))]
use std::rc::Rc;

use silex_reactivity::{
    CloseError, ErrorHandlerRef, ReactiveError, TransactionError, TransientScopeError,
};
use wasm_bindgen::JsValue;

#[cfg(feature = "error-bootstrap")]
mod bootstrap;
#[cfg(feature = "error-bootstrap")]
pub use bootstrap::{AppHostError, BootstrapError, HostState, UnmountOutcome};

#[cfg(feature = "error-dom")]
mod dom;
#[cfg(feature = "error-dom")]
pub use dom::{
    CleanupFailure, CleanupFailureDiagnostic, CleanupOrigin, CleanupReport, CleanupSink,
    DisposeError, DropFailureReport, MountAvailability, MountError, RollbackError,
};

#[cfg(feature = "error-i18n")]
mod i18n;
#[cfg(feature = "error-i18n")]
pub use i18n::{I18nError, I18nErrorKind};

#[cfg(feature = "error-intl")]
mod intl;
#[cfg(feature = "error-intl")]
pub use intl::{IntlError, IntlErrorKind};

#[cfg(feature = "error-net")]
mod net;
#[cfg(feature = "error-net")]
pub use net::{NetConnectionState, NetError, NetErrorKind};

#[cfg(feature = "error-persistence")]
mod persistence;
#[cfg(feature = "error-persistence")]
pub use persistence::{PersistenceError, PersistenceErrorKind};

#[cfg(feature = "error-router")]
mod router;
#[cfg(feature = "error-router")]
pub use router::{
    PathError, PathErrorKind, PathParamError, PathParamErrorKind, RoutePatternError,
    RoutePatternErrorKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorSeverity {
    Recoverable,
    Fatal,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Recoverable => "recoverable",
            Self::Fatal => "fatal",
        })
    }
}

#[derive(Debug, Clone)]
pub enum SilexErrorKind {
    Dom(String),
    Reactivity(ReactiveError),
    Close(CloseError),
    Transaction(Box<TransactionError>),
    Framework(String),
    Javascript(String),
    #[cfg(feature = "error-persistence")]
    Persistence(PersistenceError),
    #[cfg(feature = "error-i18n")]
    I18n(I18nError),
    #[cfg(feature = "error-router")]
    Path(PathError),
    #[cfg(feature = "error-router")]
    PathParam(PathParamError),
    #[cfg(feature = "error-router")]
    RoutePattern(RoutePatternError),
    #[cfg(feature = "error-net")]
    Net(NetError),
    #[cfg(feature = "error-intl")]
    Intl(IntlError),
    #[cfg(feature = "error-dom")]
    Mount(Rc<MountError>),
    #[cfg(feature = "error-dom")]
    Dispose(Rc<DisposeError>),
    #[cfg(feature = "error-bootstrap")]
    AppHost(Rc<AppHostError>),
    #[cfg(feature = "error-bootstrap")]
    Bootstrap(Rc<BootstrapError>),
}

impl SilexErrorKind {
    /// Return the stable category name exposed by host-facing adapters.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dom(_) => "dom",
            Self::Reactivity(_) => "reactivity",
            Self::Close(_) => "reactivity-close",
            Self::Transaction(_) => "reactivity-transaction",
            Self::Framework(_) => "framework",
            Self::Javascript(_) => "javascript",
            #[cfg(feature = "error-persistence")]
            Self::Persistence(_) => "domain",
            #[cfg(feature = "error-i18n")]
            Self::I18n(_) => "domain",
            #[cfg(feature = "error-router")]
            Self::Path(_) => "domain",
            #[cfg(feature = "error-router")]
            Self::PathParam(_) => "domain",
            #[cfg(feature = "error-router")]
            Self::RoutePattern(_) => "domain",
            #[cfg(feature = "error-net")]
            Self::Net(_) => "domain",
            #[cfg(feature = "error-intl")]
            Self::Intl(_) => "domain",
            #[cfg(feature = "error-dom")]
            Self::Mount(_) => "mount",
            #[cfg(feature = "error-dom")]
            Self::Dispose(_) => "dispose",
            #[cfg(feature = "error-bootstrap")]
            Self::AppHost(_) => "app-host",
            #[cfg(feature = "error-bootstrap")]
            Self::Bootstrap(_) => "bootstrap",
        }
    }
}

impl fmt::Display for SilexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dom(msg) => write!(f, "DOM Error: {msg}"),
            Self::Reactivity(error) => write!(f, "Reactivity Error: {error}"),
            Self::Close(error) => write!(f, "Reactivity close error: {error:?}"),
            Self::Transaction(error) => write!(f, "Reactivity transaction error: {error}"),
            Self::Framework(msg) => write!(f, "Framework Error: {msg}"),
            Self::Javascript(msg) => write!(f, "JavaScript Error: {msg}"),
            #[cfg(feature = "error-persistence")]
            Self::Persistence(error) => write!(f, "persistence error: {error}"),
            #[cfg(feature = "error-i18n")]
            Self::I18n(error) => write!(f, "i18n error: {error}"),
            #[cfg(feature = "error-router")]
            Self::Path(error) => write!(f, "route path error: {error}"),
            #[cfg(feature = "error-router")]
            Self::PathParam(error) => write!(f, "route parameter error: {error}"),
            #[cfg(feature = "error-router")]
            Self::RoutePattern(error) => write!(f, "route pattern error: {error}"),
            #[cfg(feature = "error-net")]
            Self::Net(error) => write!(f, "network error: {error}"),
            #[cfg(feature = "error-intl")]
            Self::Intl(error) => write!(f, "Intl error: {error}"),
            #[cfg(feature = "error-dom")]
            Self::Mount(error) => write!(f, "mount error: {error}"),
            #[cfg(feature = "error-dom")]
            Self::Dispose(error) => write!(f, "dispose error: {error}"),
            #[cfg(feature = "error-bootstrap")]
            Self::AppHost(error) => write!(f, "application host error: {error}"),
            #[cfg(feature = "error-bootstrap")]
            Self::Bootstrap(error) => write!(f, "bootstrap error: {error}"),
        }
    }
}

impl std::error::Error for SilexErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reactivity(error) => Some(error),
            Self::Transaction(error) => Some(error.as_ref()),
            Self::Dom(_) | Self::Close(_) | Self::Framework(_) | Self::Javascript(_) => None,
            #[cfg(feature = "error-persistence")]
            Self::Persistence(error) => Some(error),
            #[cfg(feature = "error-i18n")]
            Self::I18n(error) => Some(error),
            #[cfg(feature = "error-router")]
            Self::Path(error) => Some(error),
            #[cfg(feature = "error-router")]
            Self::PathParam(error) => Some(error),
            #[cfg(feature = "error-router")]
            Self::RoutePattern(error) => Some(error),
            #[cfg(feature = "error-net")]
            Self::Net(error) => Some(error),
            #[cfg(feature = "error-intl")]
            Self::Intl(error) => Some(error),
            #[cfg(feature = "error-dom")]
            Self::Mount(error) => Some(&**error),
            #[cfg(feature = "error-dom")]
            Self::Dispose(error) => Some(&**error),
            #[cfg(feature = "error-bootstrap")]
            Self::AppHost(error) => Some(&**error),
            #[cfg(feature = "error-bootstrap")]
            Self::Bootstrap(error) => Some(&**error),
        }
    }
}

impl PartialEq for SilexErrorKind {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Dom(left), Self::Dom(right))
            | (Self::Framework(left), Self::Framework(right))
            | (Self::Javascript(left), Self::Javascript(right)) => left == right,
            (Self::Reactivity(left), Self::Reactivity(right)) => left == right,
            (Self::Close(left), Self::Close(right)) => left == right,
            (Self::Transaction(left), Self::Transaction(right)) => left == right,
            #[cfg(feature = "error-persistence")]
            (Self::Persistence(left), Self::Persistence(right)) => left == right,
            #[cfg(feature = "error-i18n")]
            (Self::I18n(left), Self::I18n(right)) => left == right,
            #[cfg(feature = "error-router")]
            (Self::Path(left), Self::Path(right)) => left == right,
            #[cfg(feature = "error-router")]
            (Self::PathParam(left), Self::PathParam(right)) => left == right,
            #[cfg(feature = "error-router")]
            (Self::RoutePattern(left), Self::RoutePattern(right)) => left == right,
            #[cfg(feature = "error-net")]
            (Self::Net(left), Self::Net(right)) => left == right,
            #[cfg(feature = "error-intl")]
            (Self::Intl(left), Self::Intl(right)) => left == right,
            #[cfg(feature = "error-dom")]
            (Self::Mount(left), Self::Mount(right)) => Rc::ptr_eq(left, right),
            #[cfg(feature = "error-dom")]
            (Self::Dispose(left), Self::Dispose(right)) => Rc::ptr_eq(left, right),
            #[cfg(feature = "error-bootstrap")]
            (Self::AppHost(left), Self::AppHost(right)) => Rc::ptr_eq(left, right),
            #[cfg(feature = "error-bootstrap")]
            (Self::Bootstrap(left), Self::Bootstrap(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for SilexErrorKind {}

impl From<ReactiveError> for SilexErrorKind {
    fn from(error: ReactiveError) -> Self {
        Self::Reactivity(error)
    }
}

impl From<TransientScopeError> for SilexErrorKind {
    fn from(error: TransientScopeError) -> Self {
        match error {
            TransientScopeError::Runtime(error) => Self::Reactivity(error),
            TransientScopeError::Close(error) => Self::Close(error),
        }
    }
}

impl From<JsValue> for SilexErrorKind {
    fn from(value: JsValue) -> Self {
        let msg = value.as_string().unwrap_or_else(|| format!("{:?}", value));
        Self::Javascript(msg)
    }
}

#[derive(Debug, Clone)]
pub enum SilexError {
    Recoverable(SilexErrorKind),
    Fatal(SilexErrorKind),
}

impl SilexError {
    pub fn recoverable(kind: impl Into<SilexErrorKind>) -> Self {
        Self::Recoverable(kind.into())
    }

    pub fn fatal(kind: impl Into<SilexErrorKind>) -> Self {
        Self::Fatal(kind.into())
    }

    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Recoverable(_))
    }

    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal(_))
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Recoverable(_) => ErrorSeverity::Recoverable,
            Self::Fatal(_) => ErrorSeverity::Fatal,
        }
    }

    pub fn kind(&self) -> &SilexErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }

    pub fn into_kind(self) -> SilexErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }

    pub fn into_fatal(self) -> Self {
        Self::Fatal(self.into_kind())
    }
}

#[cfg(feature = "error-persistence")]
impl From<PersistenceError> for SilexError {
    fn from(error: PersistenceError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-i18n")]
impl From<I18nError> for SilexError {
    fn from(error: I18nError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-router")]
impl From<PathError> for SilexError {
    fn from(error: PathError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-router")]
impl From<PathParamError> for SilexError {
    fn from(error: PathParamError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-router")]
impl From<RoutePatternError> for SilexError {
    fn from(error: RoutePatternError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-net")]
impl From<NetError> for SilexError {
    fn from(error: NetError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-intl")]
impl From<IntlError> for SilexError {
    fn from(error: IntlError) -> Self {
        error.into_silex_error()
    }
}

#[cfg(feature = "error-dom")]
impl From<MountError> for SilexError {
    fn from(error: MountError) -> Self {
        Self::Fatal(SilexErrorKind::Mount(Rc::new(error)))
    }
}

#[cfg(feature = "error-dom")]
impl From<DisposeError> for SilexError {
    fn from(error: DisposeError) -> Self {
        Self::Fatal(SilexErrorKind::Dispose(Rc::new(error)))
    }
}

#[cfg(feature = "error-bootstrap")]
impl From<AppHostError> for SilexError {
    fn from(error: AppHostError) -> Self {
        Self::Fatal(SilexErrorKind::AppHost(Rc::new(error)))
    }
}

#[cfg(feature = "error-bootstrap")]
impl From<BootstrapError> for SilexError {
    fn from(error: BootstrapError) -> Self {
        Self::Fatal(SilexErrorKind::Bootstrap(Rc::new(error)))
    }
}

impl fmt::Display for SilexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recoverable(kind) => write!(f, "Recoverable: {kind}"),
            Self::Fatal(kind) => write!(f, "Fatal: {kind}"),
        }
    }
}

impl std::error::Error for SilexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind())
    }
}

pub type SilexResult<T> = Result<T, SilexError>;

/// Non-owning handler view for framework error dispatch.
pub type ErrorHandler<'scope> = ErrorHandlerRef<'scope, SilexError>;

/// A copyable error dispatch capability for framework-level errors.
pub type ErrorReporter<'scope> = ErrorHandler<'scope>;

/// The RAII owner for one framework error callback registration.
pub type ErrorHandlerToken<'scope> = silex_reactivity::ErrorHandlerToken<'scope, SilexError>;

/// An owning handler reference retained by a framework lifecycle context.
pub type ErrorHandlerAnchor<'scope> = silex_reactivity::ErrorHandlerAnchor<'scope, SilexError>;

/// An owning runtime lease for one framework error callback registration.
pub type HandlerLease<'scope> = silex_reactivity::HandlerLease<'scope, SilexError>;

/// A framework-level handler input accepted by computation and cleanup APIs.
#[doc(hidden)]
pub trait ErrorHandlerInput<'scope> {
    fn handler_ref(&self) -> ErrorHandler<'scope>;
}

impl<'scope> ErrorHandlerInput<'scope> for ErrorHandlerToken<'scope> {
    fn handler_ref(&self) -> ErrorHandler<'scope> {
        self.view()
    }
}

impl<'scope> ErrorHandlerInput<'scope> for ErrorHandlerAnchor<'scope> {
    fn handler_ref(&self) -> ErrorHandler<'scope> {
        self.view()
    }
}

impl<'scope> ErrorHandlerInput<'scope> for ErrorHandler<'scope> {
    fn handler_ref(&self) -> ErrorHandler<'scope> {
        *self
    }
}

impl<'scope, T> ErrorHandlerInput<'scope> for &T
where
    T: ErrorHandlerInput<'scope> + ?Sized,
{
    fn handler_ref(&self) -> ErrorHandler<'scope> {
        T::handler_ref(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{SilexError, SilexErrorKind};
    use silex_reactivity::ReactiveError;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen::JsValue;

    #[test]
    fn strategy_and_kind_are_independent() {
        let recoverable =
            SilexError::recoverable(SilexErrorKind::Framework("child rejected".to_string()));
        assert!(recoverable.is_recoverable());
        assert!(!recoverable.is_fatal());
        assert!(matches!(
            recoverable.kind(),
            SilexErrorKind::Framework(message) if message == "child rejected"
        ));

        let fatal = recoverable.into_fatal();
        assert!(fatal.is_fatal());
        assert!(!fatal.is_recoverable());
        assert!(matches!(
            fatal.kind(),
            SilexErrorKind::Framework(message) if message == "child rejected"
        ));
        assert!(fatal.clone().into_fatal().is_fatal());
    }

    #[test]
    fn error_chain_preserves_kind_and_reactivity_source() {
        let error = SilexError::fatal(SilexErrorKind::Reactivity(ReactiveError::RuntimeMismatch));
        let kind = std::error::Error::source(&error).expect("kind should be the source");
        assert!(matches!(
            kind.downcast_ref::<SilexErrorKind>(),
            Some(SilexErrorKind::Reactivity(ReactiveError::RuntimeMismatch))
        ));
        assert!(kind.source().is_some());
    }

    #[test]
    fn native_errors_convert_only_to_kinds() {
        let reactive: SilexErrorKind = ReactiveError::NoSuchNode.into();
        assert!(matches!(reactive, SilexErrorKind::Reactivity(_)));
    }

    #[test]
    fn error_kind_names_are_stable() {
        assert_eq!(SilexErrorKind::Dom(String::new()).as_str(), "dom");
        assert_eq!(
            SilexErrorKind::Reactivity(ReactiveError::NoSuchNode).as_str(),
            "reactivity"
        );
        assert_eq!(
            SilexErrorKind::Framework(String::new()).as_str(),
            "framework"
        );
        assert_eq!(
            SilexErrorKind::Javascript(String::new()).as_str(),
            "javascript"
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn javascript_errors_convert_only_to_kinds() {
        let javascript: SilexErrorKind = JsValue::from_str("bad call").into();
        assert!(matches!(javascript, SilexErrorKind::Javascript(message) if message == "bad call"));
    }
}
