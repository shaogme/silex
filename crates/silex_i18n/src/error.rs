use std::fmt::{Display, Formatter};

use silex_core::ReactiveError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum I18nError {
    InvalidLocale(String),
    InvalidCatalog(String),
    DuplicateKey(String),
    InvalidMessage { key: String, reason: String },
    MissingOther { key: String },
    Loader(String),
    Reactivity(ReactiveError),
}

impl From<ReactiveError> for I18nError {
    fn from(error: ReactiveError) -> Self {
        Self::Reactivity(error)
    }
}

impl Display for I18nError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLocale(value) => write!(f, "invalid locale: {value}"),
            Self::InvalidCatalog(reason) => write!(f, "invalid catalog: {reason}"),
            Self::DuplicateKey(key) => write!(f, "duplicate catalog key: {key}"),
            Self::InvalidMessage { key, reason } => {
                write!(f, "invalid message for {key}: {reason}")
            }
            Self::MissingOther { key } => {
                write!(f, "plural message {key} is missing the other form")
            }
            Self::Loader(reason) => write!(f, "catalog loader failed: {reason}"),
            Self::Reactivity(error) => write!(f, "reactivity error: {error}"),
        }
    }
}

impl std::error::Error for I18nError {}
