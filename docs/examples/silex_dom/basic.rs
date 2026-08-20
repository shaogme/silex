use silex_dom::element::Element;
use std::error::Error;

#[cfg(target_arch = "wasm32")]
use silex_core::{Runtime, SilexError};
#[cfg(target_arch = "wasm32")]
use silex_dom::attribute::GlobalAttributes;
#[cfg(target_arch = "wasm32")]
use silex_dom::mounted::{CleanupSink, MountedApp};

pub fn run() -> Result<(), Box<dyn Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        let host = silex_dom::document()
            .create_element("div")
            .map_err(SilexError::fatal)?;
        let mut app = MountedApp::new(Runtime::new(), host.into(), CleanupSink::new(|_| {}));

        app.mount(|context| {
            let error_handler = context.access().error_handler(|_: SilexError| {})?;
            let view = Element::with_child("button", "Hello from silex_dom").id("example");
            context.mount(view, error_handler)
        })?;
        app.dispose()?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let _ = Element::with_child("button", "Hello from silex_dom");

    Ok(())
}
