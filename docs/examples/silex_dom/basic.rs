use std::error::Error;

#[cfg(target_arch = "wasm32")]
use silex_core::{Runtime, SilexError};
#[cfg(target_arch = "wasm32")]
use silex_dom::adapters::browser::BrowserDom;
#[cfg(target_arch = "wasm32")]
use silex_dom::diagnostics::DomError;
#[cfg(target_arch = "wasm32")]
use silex_dom::lifecycle::CleanupSink;
#[cfg(target_arch = "wasm32")]
use silex_view::attributes::GlobalAttributes;
#[cfg(target_arch = "wasm32")]
use silex_view::{app::MountedApp, elements::Element};

pub fn run() -> Result<(), Box<dyn Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        let browser = BrowserDom::from_window()?;
        let host = browser.context().document_body()?.ok_or_else(|| {
            SilexError::from(DomError::Backend {
                operation: "document_body",
                message: "document body is unavailable".to_string(),
            })
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
    let _ = silex_view::elements::Element::with_child("button", "Hello from silex_view");

    Ok(())
}
