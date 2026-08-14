use super::{ErrorSeverity, SilexError};
use std::fmt;

macro_rules! define_router_error {
    ($error:ident, $kind:ident, $silex_variant:ident { $($variant:ident $(($($field:ty),*))?),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $kind {
            $($variant $(($($field),*))?),*
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $error {
            Recoverable($kind),
            Fatal($kind),
        }

        impl $error {
            pub fn recoverable(kind: $kind) -> Self { Self::Recoverable(kind) }
            pub fn fatal(kind: $kind) -> Self { Self::Fatal(kind) }
            pub fn is_recoverable(&self) -> bool { matches!(self, Self::Recoverable(_)) }
            pub fn is_fatal(&self) -> bool { matches!(self, Self::Fatal(_)) }
            pub fn severity(&self) -> ErrorSeverity {
                match self { Self::Recoverable(_) => ErrorSeverity::Recoverable, Self::Fatal(_) => ErrorSeverity::Fatal }
            }
            pub fn kind(&self) -> &$kind {
                match self { Self::Recoverable(kind) | Self::Fatal(kind) => kind }
            }
            pub fn into_kind(self) -> $kind {
                match self { Self::Recoverable(kind) | Self::Fatal(kind) => kind }
            }
            pub(crate) fn into_silex_error(self) -> SilexError {
                match self {
                    Self::Recoverable(kind) => SilexError::Recoverable(super::SilexErrorKind::$silex_variant(Self::Recoverable(kind))),
                    Self::Fatal(kind) => SilexError::Fatal(super::SilexErrorKind::$silex_variant(Self::Fatal(kind))),
                }
            }
        }

        impl std::error::Error for $kind {}

        impl fmt::Display for $error {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}: {}", self.severity(), self.kind())
            }
        }

        impl std::error::Error for $error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(self.kind()) }
        }
    };
}

define_router_error!(PathError, PathErrorKind, Path {
    InvalidPath(String),
    InvalidPercentEncoding,
    InvalidUtf8,
});

define_router_error!(PathParamError, PathParamErrorKind, PathParam {
    InvalidPercentEncoding,
    InvalidUtf8,
    InvalidValue(String),
});

impl fmt::Display for PathErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(reason) => write!(formatter, "invalid route path: {reason}"),
            Self::InvalidPercentEncoding => formatter.write_str("invalid percent encoding"),
            Self::InvalidUtf8 => formatter.write_str("percent-decoded path is not valid UTF-8"),
        }
    }
}

impl fmt::Display for PathParamErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPercentEncoding => formatter.write_str("invalid percent encoding"),
            Self::InvalidUtf8 => formatter.write_str("percent-decoded path is not valid UTF-8"),
            Self::InvalidValue(value) => write!(formatter, "invalid path parameter: {value}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutePatternErrorKind {
    Path(PathError),
    InvalidPattern { pattern: String, reason: String },
    DuplicateParameter { pattern: String, name: String },
    DuplicatePattern { pattern: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutePatternError {
    Recoverable(RoutePatternErrorKind),
    Fatal(RoutePatternErrorKind),
}

impl RoutePatternError {
    pub fn recoverable(kind: RoutePatternErrorKind) -> Self {
        Self::Recoverable(kind)
    }
    pub fn fatal(kind: RoutePatternErrorKind) -> Self {
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
    pub fn kind(&self) -> &RoutePatternErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }
    pub fn into_kind(self) -> RoutePatternErrorKind {
        match self {
            Self::Recoverable(kind) | Self::Fatal(kind) => kind,
        }
    }
    pub(crate) fn into_silex_error(self) -> SilexError {
        match self {
            Self::Recoverable(kind) => SilexError::Recoverable(
                super::SilexErrorKind::RoutePattern(Self::Recoverable(kind)),
            ),
            Self::Fatal(kind) => {
                SilexError::Fatal(super::SilexErrorKind::RoutePattern(Self::Fatal(kind)))
            }
        }
    }
}

impl fmt::Display for RoutePatternErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(error) => write!(formatter, "{error}"),
            Self::InvalidPattern { pattern, reason } => {
                write!(formatter, "invalid route pattern '{pattern}': {reason}")
            }
            Self::DuplicateParameter { pattern, name } => write!(
                formatter,
                "route pattern '{pattern}' repeats parameter '{name}'"
            ),
            Self::DuplicatePattern { pattern } => {
                write!(formatter, "route pattern '{pattern}' is duplicated")
            }
        }
    }
}

impl std::error::Error for RoutePatternErrorKind {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Path(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for RoutePatternError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.severity(), self.kind())
    }
}

impl std::error::Error for RoutePatternError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.kind())
    }
}

impl From<PathError> for RoutePatternError {
    fn from(error: PathError) -> Self {
        Self::Fatal(RoutePatternErrorKind::Path(error))
    }
}

impl From<PathError> for PathParamError {
    fn from(error: PathError) -> Self {
        let severity = error.severity();
        let kind = match error.into_kind() {
            super::router::PathErrorKind::InvalidPercentEncoding => {
                PathParamErrorKind::InvalidPercentEncoding
            }
            super::router::PathErrorKind::InvalidUtf8 => PathParamErrorKind::InvalidUtf8,
            super::router::PathErrorKind::InvalidPath(reason) => {
                PathParamErrorKind::InvalidValue(reason)
            }
        };
        match severity {
            ErrorSeverity::Recoverable => PathParamError::Recoverable(kind),
            ErrorSeverity::Fatal => PathParamError::Fatal(kind),
        }
    }
}
