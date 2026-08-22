use silex::prelude::*;
use std::error::Error;

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let error_handler = owner.error_handler(|_| {})?;
            let ctx = SilexContext::new(owner, error_handler.view());
            let visible = owner.signal(true)?;

            let content = Show(ctx, visible)
                .children("content")
                .fallback("fallback")
                .build();
            let page = div(chain!(content, p("The view is ready to mount.")));

            let _ = page;
            visible.set(false)?;
            Ok::<(), SilexError>(())
        })
        .map_err(|error| Box::new(error) as Box<dyn Error>)??;

    Ok(())
}
