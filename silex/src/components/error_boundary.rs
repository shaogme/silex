use silex_core::error::{ErrorContext, SilexError};
use silex_core::reactivity::{Effect, Signal, provide_context};
use silex_core::traits::{RxGet, RxWrite};
use silex_dom::attribute::GlobalAttributes;
use silex_dom::view::{ApplyAttributes, AutoReactiveView, Mount, MountRef};
use silex_html::div;
use std::rc::Rc;
use web_sys::Node;

/// ErrorBoundary 组件属性
pub struct ErrorBoundaryProps<F, C> {
    /// 发生错误时渲染的降级 UI，接收错误对象作为参数
    pub fallback: F,
    /// 正常渲染的子组件
    pub children: C,
}

pub struct ErrorBoundaryView<F, C> {
    props: Rc<ErrorBoundaryProps<F, C>>,
}

impl<F, C> Clone for ErrorBoundaryView<F, C> {
    fn clone(&self) -> Self {
        Self {
            props: self.props.clone(),
        }
    }
}

/// 错误边界组件
///
/// 捕获从子组件树中向上冒泡的 SilexError（通过 ErrorContext）。
///
/// # Example
/// ```rust
/// use silex::prelude::*;
///
/// ErrorBoundary(ErrorBoundaryProps {
///     fallback: |err| format!("Something went wrong: {}", err),
///     children: move || {
///         // ... component that might fail ...
///         "Everything is fine"
///     }
/// });
/// ```
#[allow(non_snake_case)]
pub fn ErrorBoundary<F, C, V1, V2>(props: ErrorBoundaryProps<F, C>) -> ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: Mount + 'static,
    V2: Mount + 'static,
{
    ErrorBoundaryView {
        props: Rc::new(props),
    }
}

impl<F, C, V1, V2> ApplyAttributes for ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: Mount + 'static,
    V2: Mount + 'static,
{
}

impl<F, C, V1, V2> Mount for ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: Mount + 'static,
    V2: Mount + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        self.mount_internal(parent, attrs);
    }
}

impl<F, C, V1, V2> AutoReactiveView for ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: Mount + 'static,
    V2: Mount + 'static,
{
}

impl<F, C, V1, V2> MountRef for ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: Mount + 'static,
    V2: Mount + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        self.clone().mount_internal(parent, attrs);
    }
}

impl<F, C, V1, V2> ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: Mount + 'static,
    V2: Mount + 'static,
{
    fn mount_internal(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        let (error, set_error) = Signal::<Option<SilexError>>::pair(None);

        provide_context(ErrorContext(Rc::new(move |e| {
            silex_core::log::console_error(format!("ErrorBoundary caught error: {}", e));
            // Defer update to avoid render-induced updates
            wasm_bindgen_futures::spawn_local(async move {
                set_error.set(Some(e));
            });
        })));

        // Create wrapper div
        let wrapper = div(()).style("display: contents");
        let wrapper_dom = wrapper.dom_element.clone();
        wrapper.mount(parent, attrs);

        let props = self.props;

        Effect::new(move |_| {
            // Clear previous content
            wrapper_dom.set_inner_html("");

            if let Some(e) = error.get() {
                (props.fallback)(e).mount(&wrapper_dom, Vec::new());
            } else {
                let process = || {
                    let view = (props.children)();
                    view.mount(&wrapper_dom, Vec::new());
                };

                if let Err(payload) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(process))
                {
                    let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                        format!("Panic: {}", s)
                    } else if let Some(s) = payload.downcast_ref::<String>() {
                        format!("Panic: {}", s)
                    } else {
                        "Unknown Panic".to_string()
                    };
                    silex_core::log::console_error(format!("ErrorBoundary caught panic: {}", msg));

                    let err = SilexError::Javascript(msg);
                    wasm_bindgen_futures::spawn_local(async move {
                        set_error.set(Some(err));
                    });
                }
            }
        });
    }
}
