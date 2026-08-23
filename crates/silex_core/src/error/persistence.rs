use super::{ErrorSeverity, SilexError, SilexErrorKind};
use silex_reactivity::ReactiveError;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceErrorKind {
    BackendUnavailable,
    ReadFailed(String),
    WriteFailed(String),
    RemoveFailed(String),
    DecodeFailed { raw: String, message: String },
    EncodeFailed(String),
    InvalidConfiguration(String),
    Reactivity(ReactiveError),
    Core(Box<SilexErrorKind>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceError {
    Recoverable(PersistenceErrorKind),
    Fatal(PersistenceErrorKind),
}

impl PersistenceError {
    pub fn recoverable(kind: PersistenceErrorKind) -> Self {
        Self::Recoverable(kind)
    }

    pub fn fatal(kind: PersistenceErrorKind) -> Self {
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

    pub fn kind(&self) -> &PersistenceErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }

    pub fn into_kind(self) -> PersistenceErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }

    pub fn message(&self) -> String {
        self.kind().to_string()
    }

    pub(crate) fn into_silex_error(self) -> SilexError {
        match self {
            Self::Recoverable(kind) => {
                SilexError::Recoverable(SilexErrorKind::Persistence(Self::Recoverable(kind)))
            }
            Self::Fatal(kind) => SilexError::Fatal(SilexErrorKind::Persistence(Self::Fatal(kind))),
        }
    }
}

impl From<ReactiveError> for PersistenceError {
    fn from(error: ReactiveError) -> Self {
        Self::Fatal(PersistenceErrorKind::Reactivity(error))
    }
}

impl From<SilexError> for PersistenceError {
    fn from(error: SilexError) -> Self {
        let severity = error.severity();
        let kind = match error.into_kind() {
            SilexErrorKind::Persistence(error) => error.into_kind(),
            SilexErrorKind::Reactivity(error) => PersistenceErrorKind::Reactivity(error),
            SilexErrorKind::Transaction(error) => {
                PersistenceErrorKind::Core(Box::new(SilexErrorKind::Transaction(error)))
            }
            kind => PersistenceErrorKind::Core(Box::new(kind)),
        };
        match severity {
            ErrorSeverity::Recoverable => Self::Recoverable(kind),
            ErrorSeverity::Fatal => Self::Fatal(kind),
        }
    }
}

impl fmt::Display for PersistenceErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendUnavailable => formatter.write_str("backend unavailable"),
            Self::ReadFailed(message)
            | Self::WriteFailed(message)
            | Self::RemoveFailed(message)
            | Self::EncodeFailed(message)
            | Self::InvalidConfiguration(message) => formatter.write_str(message),
            Self::DecodeFailed { message, .. } => formatter.write_str(message),
            Self::Reactivity(error) => write!(formatter, "{error}"),
            Self::Core(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PersistenceErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reactivity(error) => Some(error),
            Self::Core(error) => Some(&**error),
            _ => None,
        }
    }
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.severity(), self.kind())
    }
}

impl std::error::Error for PersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind())
    }
}
