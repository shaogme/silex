use std::error::Error;

#[cfg(target_arch = "wasm32")]
use silex_core::{Runtime, SilexContext, SilexError};
#[cfg(target_arch = "wasm32")]
use silex_css::prelude::*;
#[cfg(target_arch = "wasm32")]
use silex_dom::adapters::browser::BrowserDom;
#[cfg(target_arch = "wasm32")]
use silex_dom::diagnostics::DomError;
#[cfg(target_arch = "wasm32")]
use silex_dom::lifecycle::CleanupSink;
#[cfg(target_arch = "wasm32")]
use silex_view::attribute::GlobalAttributes;
#[cfg(target_arch = "wasm32")]
use silex_view::{Element, MountedApp};

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
            let error_handler = context.access().error_handler(|_| {})?;
            let style = sty(SilexContext::new(context.access(), error_handler.view()))
                .display(DisplayKeyword::Flex)?
                .gap(px(8))?
                .color(rgb(29, 78, 216))?
                .on_hover(|style| style.color(rgb(30, 64, 175)))?;
            context.mount_unit(
                Element::with_child("button", "Styled button").style(style),
                error_handler,
            )
        })?;
        app.dispose()?;
    }

    Ok(())
}
