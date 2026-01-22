use silex::prelude::*;
use silex::dom::tag::*;
use silex_macros::component;

pub fn main() {
    silex::dom::setup_global_error_handlers();
    silex::dom::element::mount_to_body(App::new());
}

#[component]
fn App() -> impl View {
    div().style("padding: 20px; font-family: sans-serif;").child((
        h1().text("Error Boundary Demo"),
        p().text("This example demonstrates how ErrorBoundary catches errors."),
        
        // 1. Recoverable Error (Result::Err) behavior
        div().style("margin-bottom: 20px; border: 1px solid #ccc; padding: 10px;").child((
            h2().text("1. Recoverable Error Test"),
            ErrorBoundary(ErrorBoundaryProps {
                fallback: |err| {
                    div().style("background-color: #fee; border: 1px solid red; padding: 10px; color: red;")
                        .child((
                            h3().text("Caught Recoverable Error!"),
                            p().text(format!("Error info: {}", err)),
                            button().text("Reset (Reload Page)").on_click(|_| {
                                let _ = web_sys::window().unwrap().location().reload();
                            })
                        ))
                },
                children: || {
                    // 无参数组件直接调用，不需要传递 Props
                    RecoverableComponent::new()
                }
            }),
        )),

        // 2. Immediate Panic Test
        div().style("margin-bottom: 20px; border: 1px solid #ccc; padding: 10px;").child((
            h2().text("2. Immediate Panic Test (Render Phase)"),
            p().text("Component below will panic completely upon rendering if triggered."),
            ErrorBoundary(ErrorBoundaryProps {
                fallback: |err| {
                    div().style("background-color: #fff3cd; border: 1px solid orange; padding: 10px; color: #856404;")
                        .child((
                            h3().text("Caught Panic!"),
                            p().text(format!("Panic details: {}", err)),
                        ))
                },
                children: || {
                    // 无参数组件直接调用
                    PanicToggleComponent::new()
                }
            }),
        )),
    ))
}

#[component]
fn RecoverableComponent() -> impl View {
    let (should_error, set_should_error) = create_signal(false);
    
    move || {
        if should_error.get() {
             // Return an Err, which triggers handle_error -> ErrorContext
              Err(SilexError::Javascript("User clicked the error button!".into()))
        } else {
             Ok(div().child((
                 p().text("Component is running normally."),
                 button().text("Trigger Result::Err").on_click(move |_| {
                     set_should_error.set(true);
                 })
             )))
        }
    }
}

// A component that conditionally renders a child that panics immediately during construction
#[component]
fn PanicToggleComponent() -> impl View {
     let (show_panic, _set_show_panic) = create_signal(false);
     
     move || {
         if show_panic.get() {
             // We wrap the panicking component in a way that its construction is delayed until this closure runs
             // Because ErrorBoundary wraps this closure in create_effect and catch_unwind, it captures this panic.
             // 无参数组件直接调用
             Some(ImmediatePanic::new())
         } else {
             None
         }
     }
}

#[component]
fn ImmediatePanic() -> impl View {
    let (active, set_active) = create_signal(false);
    
    div().child((
         p().text("Ready to panic?"),
         button().text("Click to Panic Immediately").on_click(move |_| {
              set_active.set(true);
         }),
         // This closure runs inside an effect.
         // Silex View wrapper now implements catch_unwind within the reactive effect,
         // so this panic SHOULD be caught by the ErrorBoundary.
         move || {
             if active.get() {
                 panic!("KA-BOOM! Panic in render function.");
             }
             "Safe"
         }
    ))
}
