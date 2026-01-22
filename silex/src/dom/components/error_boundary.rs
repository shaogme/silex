use crate::dom::element::tag::div;
use crate::dom::view::View;
use crate::error::{ErrorContext, SilexError};
use crate::reactivity::{create_effect, create_signal, provide_context};
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
    props: ErrorBoundaryProps<F, C>,
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
/// })
/// ```
#[allow(non_snake_case)]
pub fn ErrorBoundary<F, C, V1, V2>(props: ErrorBoundaryProps<F, C>) -> ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: View + 'static,
    V2: View + 'static,
{
    ErrorBoundaryView { props }
}

impl<F, C, V1, V2> View for ErrorBoundaryView<F, C>
where
    F: Fn(SilexError) -> V1 + 'static,
    C: Fn() -> V2 + 'static,
    V1: View + 'static,
    V2: View + 'static,
{
    fn mount(self, parent: &Node) {
        let (error, set_error) = create_signal::<Option<SilexError>>(None);

        provide_context(ErrorContext(Rc::new(move |e| {
            crate::logging::console_error(&format!("ErrorBoundary caught error: {}", e));
            // Defer update to avoid render-induced updates
            wasm_bindgen_futures::spawn_local(async move {
                set_error.set(Some(e));
            });
        })))
        .unwrap_or_else(|e| {
            crate::logging::console_error(&format!("Error providing context: {}", e))
        });

        // Create wrapper div
        // We use "display: contents" so it doesn't affect layout if supported
        let wrapper = div().style("display: contents");

        let wrapper_dom = wrapper.dom_element.clone();
        wrapper.mount(parent);

        let props = self.props;

        create_effect(move || {
            // Clear previous content
            wrapper_dom.set_inner_html("");

            if let Some(e) = error.get().flatten() {
                (props.fallback)(e).mount(&wrapper_dom);
            } else {
                // Catch panic
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(&props.children));
                match result {
                    Ok(view) => view.mount(&wrapper_dom),
                    Err(payload) => {
                        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                            format!("Panic: {}", s)
                        } else if let Some(s) = payload.downcast_ref::<String>() {
                            format!("Panic: {}", s)
                        } else {
                            "Unknown Panic".to_string()
                        };
                        crate::logging::console_error(&format!(
                            "ErrorBoundary caught panic: {}",
                            msg
                        ));

                        let err = SilexError::Javascript(msg);
                        // Trigger re-run to show fallback
                        // Defer update to avoid render-induced updates
                        wasm_bindgen_futures::spawn_local(async move {
                            set_error.set(Some(err));
                        });
                    }
                }
            }
        });
    }
}
