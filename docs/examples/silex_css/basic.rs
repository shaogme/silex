use silex_core::{ErrorHandlerToken, OwnerAccess, Runtime, SilexContext, SilexError, SilexResult};
use silex_css::prelude::*;
use std::error::Error;

fn handler<'scope>(owner: OwnerAccess<'scope>) -> SilexResult<ErrorHandlerToken<'scope>> {
    owner.error_handler(|_error| {})
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let error_handler = handler(owner)?;
            let style = sty(SilexContext::new(owner, error_handler.view()))
                .display(DisplayKeyword::Flex)?
                .gap(px(8))?
                .color(rgb(29, 78, 216))?
                .on_hover(|style| style.color(rgb(30, 64, 175)))?;

            #[cfg(target_arch = "wasm32")]
            {
                use silex_dom::view::MountOwnerToken;

                let element = silex_dom::document()
                    .create_element("button")
                    .map_err(SilexError::fatal)?;
                let owner_token = MountOwnerToken::new(owner);
                style.apply_to_element(&element, &owner_token, error_handler.view())?;
            }

            #[cfg(not(target_arch = "wasm32"))]
            let _ = style;

            Ok::<(), SilexError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}
