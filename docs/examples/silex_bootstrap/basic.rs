use std::error::Error;

pub fn run() -> Result<(), Box<dyn Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        use silex_bootstrap::AppHost;
        use silex_core::{BootstrapError, Runtime, SilexError};
        use silex_dom::lifecycle::CleanupSink;
        use silex_view::elements::Element;
        use web_sys::Node;

        let window = web_sys::window().ok_or_else(|| {
            SilexError::from(BootstrapError::TargetNotFound("window".to_string()))
        })?;
        let document = window.document().ok_or_else(|| {
            SilexError::from(BootstrapError::TargetNotFound("document".to_string()))
        })?;
        let target: Node = document
            .create_element("div")
            .map_err(SilexError::fatal)?
            .into();
        let mut host = AppHost::from_web_sys(target, CleanupSink::new(|_| {}))?;

        host.mount(Runtime::new(), |context| {
            let handler = context.access().error_handler(|_: SilexError| {})?;
            context.mount_unit(
                Element::with_child("main", "Hello from silex_bootstrap"),
                handler,
            )
        })?;
        host.unmount()?;
    }

    Ok(())
}
