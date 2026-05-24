use silex_core::error::{ErrorContext, SilexError};
use silex_core::reactivity::Signal;
use silex_core::traits::{RxGet, RxWrite};
use silex_dom::prelude::*;
use silex_macros::{component, render};
use std::rc::Rc;

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
        silex_core::log::console_error(format!("ErrorBoundary caught error: {}", e));
        // Defer update to avoid render-induced updates
        wasm_bindgen_futures::spawn_local(async move {
            set_error.set(Some(e));
        });
    }));

    render! {
        use scope;
        use provide error_ctx;

        let fallback = fallback.clone();
        let children = children.clone();

        silex_core::rx! {
            if let Some(e) = error.get() {
                fallback(e).into_any()
            } else {
                let res =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe({
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
                        silex_core::log::console_error(format!(
                            "ErrorBoundary caught panic: {}",
                            msg
                        ));

                        let err = SilexError::Javascript(msg);
                        wasm_bindgen_futures::spawn_local(async move {
                            set_error.set(Some(err));
                        });
                        AnyView::Empty
                    }
                }
            }
        }
    }
}
