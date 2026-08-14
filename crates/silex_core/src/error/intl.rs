use super::{ErrorSeverity, SilexError};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntlErrorKind {
    InvalidValue(String),
    JavaScript(String),
    Unsupported(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntlError {
    Recoverable(IntlErrorKind),
    Fatal(IntlErrorKind),
}

impl IntlError {
    pub fn recoverable(kind: IntlErrorKind) -> Self {
        Self::Recoverable(kind)
    }
    pub fn fatal(kind: IntlErrorKind) -> Self {
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
    pub fn kind(&self) -> &IntlErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }
    pub fn into_kind(self) -> IntlErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }
    pub(crate) fn into_silex_error(self) -> SilexError {
        match self {
            Self::Recoverable(kind) => {
                SilexError::Recoverable(super::SilexErrorKind::Intl(Self::Recoverable(kind)))
            }
            Self::Fatal(kind) => SilexError::Fatal(super::SilexErrorKind::Intl(Self::Fatal(kind))),
        }
    }
}

impl fmt::Display for IntlErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(value) => write!(formatter, "invalid Intl value: {value}"),
            Self::JavaScript(reason) => write!(formatter, "Intl operation failed: {reason}"),
            Self::Unsupported(formatter_name) => {
                write!(formatter, "Intl formatter is unsupported: {formatter_name}")
            }
        }
    }
}
impl std::error::Error for IntlErrorKind {}
impl fmt::Display for IntlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.severity(), self.kind())
    }
}
impl std::error::Error for IntlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind())
    }
}
