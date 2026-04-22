use silex_core::error::{ErrorContext, SilexError};
use silex_core::reactivity::{ReadSignal, Signal, WriteSignal, provide_context};
use silex_core::traits::{RxGet, RxWrite};
use silex_dom::prelude::*;
use silex_html::div;
use silex_macros::component;
use std::rc::Rc;

/// ErrorBoundary 组件
///
/// 捕获从子组件树中向上冒泡的 SilexError（通过 ErrorContext）。
#[component]
pub fn ErrorBoundary<FB, CH, V1, V2>(
    #[prop(clone)] children: CH,
    #[prop(clone)] fallback: FB,
) -> impl View
where
    FB: Fn(SilexError) -> V1 + Clone + 'static,
    CH: Fn() -> V2 + Clone + 'static,
    V1: View + 'static,
    V2: View + 'static,
{
    let (error, set_error) = Signal::<Option<SilexError>>::pair(None);

    provide_context(ErrorContext(Rc::new(move |e| {
        silex_core::log::console_error(format!("ErrorBoundary caught error: {}", e));
        // Defer update to avoid render-induced updates
        wasm_bindgen_futures::spawn_local(async move {
            set_error.set(Some(e));
        });
    })));

    // 使用专用包装视图以解决闭包生命周期与 Prop 生命周期绑定的问题
    ErrorBoundaryView {
        children: Rc::new(move || children().into_any()),
        fallback: Rc::new(move |e| fallback(e).into_any()),
        error,
        set_error,
    }
}

struct ErrorBoundaryView {
    children: Rc<dyn Fn() -> AnyView + 'static>,
    fallback: Rc<dyn Fn(SilexError) -> AnyView + 'static>,
    error: ReadSignal<Option<SilexError>>,
    set_error: WriteSignal<Option<SilexError>>,
}

impl ApplyAttributes for ErrorBoundaryView {}

impl View for ErrorBoundaryView {
    fn mount(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        let error = self.error;
        let set_error = self.set_error;
        let fallback = self.fallback.clone();
        let children = self.children.clone();

        div(move || {
            let fallback = fallback.clone();
            let children = children.clone();

            if let Some(e) = error.get() {
                fallback(e)
            } else {
                let res =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || children()));

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
                        ().into_any()
                    }
                }
            }
        })
        .style("display: contents")
        .mount(parent, attrs);
    }
}

impl AutoReactiveView for ErrorBoundaryView {}
