use std::error::Error;

pub fn run() -> Result<(), Box<dyn Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        use silex_bootstrap::AppHost;
        use silex_core::{Runtime, SilexError};
        use silex_dom::{CleanupSink, element::Element};
        use web_sys::Node;

        let target: Node = silex_dom::document()
            .create_element("div")
            .map_err(SilexError::fatal)?
            .into();
        let mut host = AppHost::new(target, CleanupSink::new(|_| {}));

        host.mount(Runtime::new(), |context| {
            let handler = context.access().error_handler(|_: SilexError| {})?;
            context.mount(
                Element::with_child("main", "Hello from silex_bootstrap"),
                handler,
            )
        })?;
        host.unmount()?;
    }

    Ok(())
}
