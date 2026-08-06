use std::panic::{AssertUnwindSafe, catch_unwind};

use wasm_bindgen_futures::spawn_local;

use silex_core::{
    error::{ErrorContext, SilexError},
    log::console_error,
    Scope,
    rx,
};
use silex_dom::prelude::*;
use silex_macros::component;

/// ErrorBoundary 组件
///
/// 捕获从子组件树中向上冒泡的 SilexError（通过 ErrorContext）。
#[component]
pub fn ErrorBoundary<'scope, FB, CH, V1, V2>(
    scope: Scope<'scope>,
    children: CH,
    #[chain] fallback: FB,
) -> impl View<'scope>
where
    FB: Fn(SilexError) -> V1 + Clone + 'scope,
    CH: Fn() -> V2 + Clone + 'scope,
    V1: View<'scope> + 'scope,
    V2: View<'scope> + 'scope,
{
    let (error, set_error) = scope.signal(None::<SilexError>);
    let completion = scope.completion_sender(move |value| set_error.set(Some(value)));
    let completion_for_context = completion.clone();

    let error_ctx = ErrorContext::new(move |e| {
        console_error(format!("ErrorBoundary caught error: {}", e));
        let completion = completion_for_context.clone();
        spawn_local(async move {
            let _ = completion.submit(e);
        });
    });

    error_ctx.push(scope);

    let fallback = fallback.clone();
    let children = children.clone();
    rx!(scope; {
        if let Some(e) = (*$error).clone() {
            fallback(e).into_any()
        } else {
            let result = catch_unwind(AssertUnwindSafe({
                let children = children.clone();
                move || children().into_any()
            }));

            match result {
                Ok(view) => view,
                Err(payload) => {
                    let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                        format!("Panic: {}", s)
                    } else if let Some(s) = payload.downcast_ref::<String>() {
                        format!("Panic: {}", s)
                    } else {
                        "Unknown Panic".to_string()
                    };
                    console_error(format!("ErrorBoundary caught panic: {}", msg));

                    let completion = completion.clone();
                    let error = SilexError::Javascript(msg);
                    spawn_local(async move {
                        let _ = completion.submit(error);
                    });
                    AnyView::Empty
                }
            }
        }
    })
}
