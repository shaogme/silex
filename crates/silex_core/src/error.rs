use std::fmt;

#[derive(Debug, Clone)] // Clone to allow easy propagation in closures if needed
pub enum SilexError {
    Dom(String),
    Reactivity(String),
    Javascript(String),
}

#[derive(Clone)]
pub struct ErrorContext(pub std::rc::Rc<dyn Fn(SilexError)>);

impl fmt::Display for SilexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SilexError::Dom(msg) => write!(f, "DOM Error: {}", msg),
            SilexError::Reactivity(msg) => write!(f, "Reactivity Error: {}", msg),
            SilexError::Javascript(msg) => write!(f, "JavaScript Error: {}", msg),
        }
    }
}

impl std::error::Error for SilexError {}

impl From<wasm_bindgen::JsValue> for SilexError {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        let msg = value.as_string().unwrap_or_else(|| format!("{:?}", value));
        SilexError::Javascript(msg)
    }
}

pub type SilexResult<T> = Result<T, SilexError>;

pub fn handle_error(err: SilexError) {
    if let Some(ctx) = crate::reactivity::use_context::<ErrorContext>() {
        (ctx.0)(err);
    } else {
        crate::error!("Unhandled Silex Error: {:?}", err);
    }
}
