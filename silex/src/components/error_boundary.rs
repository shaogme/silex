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
) -> impl Mount + MountRef
where
    FB: Fn(SilexError) -> V1 + Clone + 'static,
    CH: Fn() -> V2 + Clone + 'static,
    V1: MountExt,
    V2: MountExt,
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
        children: children.clone(),
        fallback: fallback.clone(),
        error,
        set_error,
        _pd: std::marker::PhantomData,
    }
}

struct ErrorBoundaryView<FB, CH, V1, V2> {
    children: CH,
    fallback: FB,
    error: ReadSignal<Option<SilexError>>,
    set_error: WriteSignal<Option<SilexError>>,
    _pd: std::marker::PhantomData<(V1, V2)>,
}

impl<FB, CH, V1, V2> ApplyAttributes for ErrorBoundaryView<FB, CH, V1, V2> {}

impl<FB, CH, V1, V2> Mount for ErrorBoundaryView<FB, CH, V1, V2>
where
    FB: Fn(SilexError) -> V1 + Clone + 'static,
    CH: Fn() -> V2 + Clone + 'static,
    V1: MountExt,
    V2: MountExt,
{
    fn mount(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        let error = self.error;
        let set_error = self.set_error;
        let fallback = self.fallback;
        let children = self.children;

        div(move || {
            let fallback = fallback.clone();
            let children = children.clone();

            if let Some(e) = error.get() {
                fallback(e).into_any()
            } else {
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    children().into_any()
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
                        ().into_any()
                    }
                }
            }
        })
        .style("display: contents")
        .mount(parent, attrs);
    }
}

impl<FB, CH, V1, V2> AutoReactiveView for ErrorBoundaryView<FB, CH, V1, V2>
where
    FB: Fn(SilexError) -> V1 + Clone + 'static,
    CH: Fn() -> V2 + Clone + 'static,
    V1: MountExt,
    V2: MountExt,
{
}

impl<FB, CH, V1, V2> MountRef for ErrorBoundaryView<FB, CH, V1, V2>
where
    FB: Fn(SilexError) -> V1 + Clone + 'static,
    CH: Fn() -> V2 + Clone + 'static,
    V1: MountExt,
    V2: MountExt,
{
    fn mount_ref(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        ErrorBoundaryView {
            children: self.children.clone(),
            fallback: self.fallback.clone(),
            error: self.error,
            set_error: self.set_error,
            _pd: std::marker::PhantomData,
        }
        .mount(parent, attrs);
    }
}
