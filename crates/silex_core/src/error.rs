use std::{error::Error, fmt};

use silex_reactivity::{
    CloseError, ErrorHandlerAnchor as ReactiveErrorHandlerAnchor, ErrorHandlerRef,
    ErrorHandlerToken as ReactiveErrorHandlerToken, HandlerLease as ReactiveHandlerLease,
    ReactiveError, TransactionError, TransientScopeError,
};
use wasm_bindgen::JsValue;

pub mod bootstrap;
pub mod dom;
pub mod view;

pub use bootstrap::{AppHostError, BootstrapError, HostState, UnmountOutcome};
pub use dom::{CleanupFailure, CleanupOrigin, CleanupReport, DomError, DomResult};
pub use view::{DisposeError, MountAvailability, MountError, RollbackError, ViewError};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SilexErrorKind {
    Dom(DomError),
    Reactivity(ReactiveError),
    Close(CloseError),
    Transaction(Box<TransactionError>),
    Framework(String),
    Javascript(String),
    View(Box<ViewError>),
    Bootstrap(Box<BootstrapError>),
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
            Self::View(_) => "view",
            Self::Bootstrap(_) => "bootstrap",
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
        }
    }
}

impl fmt::Display for SilexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dom(error) => write!(f, "DOM Error: {error}"),
            Self::Reactivity(error) => write!(f, "Reactivity Error: {error}"),
            Self::Close(error) => write!(f, "Reactivity close error: {error:?}"),
            Self::Transaction(error) => write!(f, "Reactivity transaction error: {error}"),
            Self::Framework(msg) => write!(f, "Framework Error: {msg}"),
            Self::Javascript(msg) => write!(f, "JavaScript Error: {msg}"),
            Self::View(error) => write!(f, "View Error: {error}"),
            Self::Bootstrap(error) => write!(f, "Bootstrap Error: {error}"),
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
        }
    }
}

impl Error for SilexErrorKind {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Reactivity(error) => Some(error),
            Self::Transaction(error) => Some(error.as_ref()),
            Self::Close(_) | Self::Framework(_) | Self::Javascript(_) => None,
            Self::Dom(error) => Some(error),
            Self::View(error) => Some(error.as_ref()),
            Self::Bootstrap(error) => Some(error.as_ref()),
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
        }
    }
}

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

impl From<DomError> for SilexErrorKind {
    fn from(error: DomError) -> Self {
        Self::Dom(error)
    }
}

impl From<DomError> for SilexError {
    fn from(error: DomError) -> Self {
        Self::fatal(SilexErrorKind::Dom(error))
    }
}

impl From<JsValue> for SilexErrorKind {
    fn from(value: JsValue) -> Self {
        let msg = value.as_string().unwrap_or_else(|| format!("{:?}", value));
        Self::Javascript(msg)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub(crate) fn with_severity(kind: SilexErrorKind, severity: ErrorSeverity) -> Self {
        match severity {
            ErrorSeverity::Recoverable => Self::Recoverable(kind),
            ErrorSeverity::Fatal => Self::Fatal(kind),
        }
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

impl fmt::Display for SilexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recoverable(kind) => write!(f, "Recoverable: {kind}"),
            Self::Fatal(kind) => write!(f, "Fatal: {kind}"),
        }
    }
}

impl Error for SilexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.kind())
    }
}

pub type SilexResult<T> = Result<T, SilexError>;

/// Non-owning handler view for framework error dispatch.
pub type ErrorHandler<'scope> = ErrorHandlerRef<'scope, SilexError>;

/// A copyable error dispatch capability for framework-level errors.
pub type ErrorReporter<'scope> = ErrorHandler<'scope>;

/// The RAII owner for one framework error callback registration.
pub type ErrorHandlerToken<'scope> = ReactiveErrorHandlerToken<'scope, SilexError>;

/// An owning handler reference retained by a framework lifecycle context.
pub type ErrorHandlerAnchor<'scope> = ReactiveErrorHandlerAnchor<'scope, SilexError>;

/// An owning runtime lease for one framework error callback registration.
pub type HandlerLease<'scope> = ReactiveHandlerLease<'scope, SilexError>;

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
    use super::{
        AppHostError, BootstrapError, CleanupReport, DomError, ErrorSeverity, MountError,
        SilexError, SilexErrorKind, ViewError,
    };
    use silex_reactivity::ReactiveError;
    use std::error::Error;
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
        let kind = Error::source(&error).expect("kind should be the source");
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
    fn dom_conversion_preserves_structured_fields() {
        let dom = DomError::CrossContext {
            expected: 7,
            actual: 9,
        };
        let error = SilexError::from(dom.clone());
        assert_eq!(error.severity(), ErrorSeverity::Fatal);
        assert!(matches!(
            error.kind(),
            SilexErrorKind::Dom(DomError::CrossContext {
                expected: 7,
                actual: 9
            })
        ));
        assert_eq!(
            error.to_string(),
            "Fatal: DOM Error: DOM handles belong to different contexts (expected 7, got 9)"
        );
        assert_eq!(
            SilexErrorKind::from(dom),
            SilexErrorKind::Dom(DomError::CrossContext {
                expected: 7,
                actual: 9,
            })
        );
        assert!(SilexErrorKind::Dom(DomError::Cycle).source().is_some());
    }

    #[test]
    fn view_mount_preserves_retryable_severity_and_source_chain() {
        let primary = SilexError::recoverable(SilexErrorKind::Framework("retry".to_string()));
        let mount = MountError::new(primary.clone(), CleanupReport::new());
        let error = SilexError::from(ViewError::from(mount));

        assert_eq!(error.severity(), ErrorSeverity::Recoverable);
        let kind = error.source().expect("kind should be the source");
        let view = kind
            .downcast_ref::<SilexErrorKind>()
            .and_then(|kind| match kind {
                SilexErrorKind::View(view) => Some(view.as_ref()),
                _ => None,
            })
            .expect("view payload should be present");
        let mount = view.mount_error().expect("mount payload should be present");
        assert_eq!(mount.primary(), &primary);
        assert!(mount.source().is_some());
    }

    #[test]
    fn bootstrap_host_preserves_nested_mount_and_severity() {
        let primary = SilexError::recoverable(SilexErrorKind::Framework("retry".to_string()));
        let mount = MountError::new(primary, CleanupReport::new());
        let error = SilexError::from(AppHostError::Mount(Box::new(mount)));

        assert_eq!(error.severity(), ErrorSeverity::Recoverable);
        let bootstrap = match error.kind() {
            SilexErrorKind::Bootstrap(bootstrap) => bootstrap.as_ref(),
            _ => panic!("expected bootstrap payload"),
        };
        let host = match bootstrap {
            BootstrapError::Host(host) => host,
            _ => panic!("expected host error"),
        };
        assert!(host.mount_error().is_some());
        assert!(bootstrap.source().is_some());
    }

    #[test]
    fn structured_errors_have_value_semantics() {
        let error = SilexError::fatal(DomError::Unsupported {
            capability: "events",
        });
        assert_eq!(error.clone(), error);
        assert_ne!(
            error,
            SilexError::fatal(DomError::Unsupported { capability: "ssr" })
        );
    }

    #[test]
    fn error_kind_names_are_stable() {
        assert_eq!(SilexErrorKind::Dom(DomError::Cycle).as_str(), "dom");
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
