use std::error::Error;

#[cfg(target_arch = "wasm32")]
use silex_core::{Runtime, SilexError, SilexErrorKind};
#[cfg(target_arch = "wasm32")]
use silex_dom::browser::BrowserDom;
#[cfg(target_arch = "wasm32")]
use silex_dom::error::CleanupSink;
#[cfg(target_arch = "wasm32")]
use silex_view::attribute::GlobalAttributes;
#[cfg(target_arch = "wasm32")]
use silex_view::{Element, MountedApp};

pub fn run() -> Result<(), Box<dyn Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        let browser = BrowserDom::from_window()
            .map_err(|error| SilexError::fatal(SilexErrorKind::Dom(error.to_string())))?;
        let host = browser
            .context()
            .document_body()
            .map_err(|error| SilexError::fatal(SilexErrorKind::Dom(error.to_string())))?
            .ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom(
                    "document body is unavailable".to_string(),
                ))
            })?;
        let mut app = MountedApp::new(
            Runtime::new(),
            browser.context(),
            host.node().clone(),
            CleanupSink::new(|_| {}),
        );

        app.mount(|context| {
            let error_handler = context.access().error_handler(|_: SilexError| {})?;
            let view = Element::with_child("button", "Hello from silex_view").id("example");
            context.mount_unit(view, error_handler)
        })?;
        app.dispose()?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = silex_view::Element::with_child("button", "Hello from silex_view");

    Ok(())
}
