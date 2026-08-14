use super::{ErrorSeverity, SilexError, SilexErrorKind};
use silex_reactivity::ReactiveError;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum I18nErrorKind {
    InvalidLocale(String),
    InvalidCatalog(String),
    DuplicateKey(String),
    InvalidMessage {
        key: String,
        reason: String,
    },
    MissingOther {
        key: String,
    },
    Loader(String),
    Reactivity(ReactiveError),
    Core(Box<SilexErrorKind>),
    #[cfg(feature = "error-i18n-persistence")]
    Persistence(super::PersistenceError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum I18nError {
    Recoverable(I18nErrorKind),
    Fatal(I18nErrorKind),
}

impl I18nError {
    pub fn recoverable(kind: I18nErrorKind) -> Self {
        Self::Recoverable(kind)
    }

    pub fn fatal(kind: I18nErrorKind) -> Self {
        Self::Fatal(kind)
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

    pub fn kind(&self) -> &I18nErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }

    pub fn into_kind(self) -> I18nErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }

    pub(crate) fn into_silex_error(self) -> SilexError {
        match self {
            Self::Recoverable(kind) => {
                SilexError::Recoverable(SilexErrorKind::I18n(Self::Recoverable(kind)))
            }
            Self::Fatal(kind) => SilexError::Fatal(SilexErrorKind::I18n(Self::Fatal(kind))),
        }
    }
}

impl From<ReactiveError> for I18nError {
    fn from(error: ReactiveError) -> Self {
        Self::Fatal(I18nErrorKind::Reactivity(error))
    }
}

impl From<SilexError> for I18nError {
    fn from(error: SilexError) -> Self {
        let severity = error.severity();
        let kind = match error.into_kind() {
            SilexErrorKind::I18n(error) => error.into_kind(),
            SilexErrorKind::Reactivity(error) => I18nErrorKind::Reactivity(error),
            #[cfg(feature = "error-i18n-persistence")]
            SilexErrorKind::Persistence(error) => I18nErrorKind::Persistence(error),
            kind => I18nErrorKind::Core(Box::new(kind)),
        };
        match severity {
            ErrorSeverity::Recoverable => Self::Recoverable(kind),
            ErrorSeverity::Fatal => Self::Fatal(kind),
        }
    }
}

#[cfg(feature = "error-i18n-persistence")]
impl From<super::PersistenceError> for I18nError {
    fn from(error: super::PersistenceError) -> Self {
        let severity = error.severity();
        let kind = I18nErrorKind::Persistence(error);
        match severity {
            ErrorSeverity::Recoverable => Self::Recoverable(kind),
            ErrorSeverity::Fatal => Self::Fatal(kind),
        }
    }
}

impl fmt::Display for I18nErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLocale(value) => write!(formatter, "invalid locale: {value}"),
            Self::InvalidCatalog(reason) => write!(formatter, "invalid catalog: {reason}"),
            Self::DuplicateKey(key) => write!(formatter, "duplicate catalog key: {key}"),
            Self::InvalidMessage { key, reason } => {
                write!(formatter, "invalid message for {key}: {reason}")
            }
            Self::MissingOther { key } => {
                write!(formatter, "plural message {key} is missing the other form")
            }
            Self::Loader(reason) => write!(formatter, "catalog loader failed: {reason}"),
            Self::Reactivity(error) => write!(formatter, "reactivity error: {error}"),
            Self::Core(error) => write!(formatter, "core error: {error}"),
            #[cfg(feature = "error-i18n-persistence")]
            Self::Persistence(error) => write!(formatter, "persistence error: {error}"),
        }
    }
}

impl std::error::Error for I18nErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reactivity(error) => Some(error),
            Self::Core(error) => Some(&**error),
            #[cfg(feature = "error-i18n-persistence")]
            Self::Persistence(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for I18nError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.severity(), self.kind())
    }
}

impl std::error::Error for I18nError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind())
    }
}
