use proc_macro2::Span;
use syn::parse::Error;

pub enum CssError {
    Lightning(String, Span),
}

impl CssError {
    pub fn into_syn(self) -> Error {
        match self {
            CssError::Lightning(msg, span) => {
                Error::new(span, format!("CSS Minification/Validation Error: {}", msg))
            }
        }
    }
}

/// Helper to map LightningCSS location info back to source spans.
pub fn report_lightning_error(err: String, base_span: Span) -> Error {
    CssError::Lightning(err, base_span).into_syn()
}
