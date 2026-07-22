use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;

use silex_core::{
    error::{ErrorContext, SilexError, provide_context},
    log::console_error,
    reactivity::Signal,
    rx,
    traits::{RxGet, RxWrite},
};
use silex_dom::prelude::*;
use silex_macros::{component, render};

/// ErrorBoundary 组件
///
/// 捕获从子组件树中向上冒泡的 SilexError（通过 ErrorContext）。
#[component]
pub fn ErrorBoundary<FB, CH, V1, V2>(children: CH, #[chain] fallback: FB) -> impl View
where
    FB: Fn(SilexError) -> V1 + Clone + 'static,
    CH: Fn() -> V2 + Clone + 'static,
    V1: View + 'static,
    V2: View + 'static,
{
    let (error, set_error) = Signal::<Option<SilexError>>::pair(None);

    let error_ctx = ErrorContext(Rc::new(move |e| {
        console_error(format!("ErrorBoundary caught error: {}", e));
        // Defer update to avoid render-induced updates
        spawn_local(async move {
            set_error.set(Some(e));
        });
    }));

    provide_context(error_ctx);

    render! {
        use scope;

        let fallback = fallback.clone();
        let children = children.clone();

        rx! {
            if let Some(e) = error.get() {
                fallback(e).into_any()
            } else {
                let res = catch_unwind(AssertUnwindSafe({
                    let children = children.clone();
                    move || children().into_any()
                }));

                match res {
                    Ok(view) => view,
                    Err(payload) => {
                        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                            format!("Panic: {}", s)
                        } else if let Some(s) = payload.downcast_ref::<String>() {
                            format!("Panic: {}", s)
                        } else {
                            "Unknown Panic".to_string()
                        };
                        console_error(format!(
                            "ErrorBoundary caught panic: {}",
                            msg
                        ));

                        let err = SilexError::Javascript(msg);
                        spawn_local(async move {
                            set_error.set(Some(err));
                        });
                        AnyView::Empty
                    }
                }
            }
        }
    }
}
