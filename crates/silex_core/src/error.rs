use std::fmt;

pub use silex_reactivity::ErrorHandler;
use silex_reactivity::ReactiveError;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone)]
pub enum SilexErrorKind {
    Dom(String),
    Reactivity(ReactiveError),
    Framework(String),
    Javascript(String),
}

impl fmt::Display for SilexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dom(msg) => write!(f, "DOM Error: {msg}"),
            Self::Reactivity(error) => write!(f, "Reactivity Error: {error}"),
            Self::Framework(msg) => write!(f, "Framework Error: {msg}"),
            Self::Javascript(msg) => write!(f, "JavaScript Error: {msg}"),
        }
    }
}

impl std::error::Error for SilexErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reactivity(error) => Some(error),
            Self::Dom(_) | Self::Framework(_) | Self::Javascript(_) => None,
        }
    }
}

impl From<ReactiveError> for SilexErrorKind {
    fn from(error: ReactiveError) -> Self {
        Self::Reactivity(error)
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

pub type ErrorReporter<'scope> = ErrorHandler<'scope, SilexError>;

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

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn javascript_errors_convert_only_to_kinds() {
        let javascript: SilexErrorKind = JsValue::from_str("bad call").into();
        assert!(matches!(javascript, SilexErrorKind::Javascript(message) if message == "bad call"));
    }
}
