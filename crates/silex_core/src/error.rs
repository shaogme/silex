use std::fmt;

pub use silex_reactivity::ErrorHandler;
use silex_reactivity::ReactiveError;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone)]
pub enum SilexError {
    Dom(String),
    Reactivity(ReactiveError),
    Framework(String),
    Javascript(String),
}

impl fmt::Display for SilexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SilexError::Dom(msg) => write!(f, "DOM Error: {}", msg),
            SilexError::Reactivity(error) => write!(f, "Reactivity Error: {}", error),
            SilexError::Framework(msg) => write!(f, "Framework Error: {}", msg),
            SilexError::Javascript(msg) => write!(f, "JavaScript Error: {}", msg),
        }
    }
}

impl From<ReactiveError> for SilexError {
    fn from(error: ReactiveError) -> Self {
        Self::Reactivity(error)
    }
}

impl std::error::Error for SilexError {}

impl From<JsValue> for SilexError {
    fn from(value: JsValue) -> Self {
        let msg = value.as_string().unwrap_or_else(|| format!("{:?}", value));
        SilexError::Javascript(msg)
    }
}

pub type SilexResult<T> = Result<T, SilexError>;

pub type ErrorReporter<'scope> = ErrorHandler<'scope, SilexError>;
