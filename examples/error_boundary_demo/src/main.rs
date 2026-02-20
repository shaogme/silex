use silex::prelude::*;
use silex::reexports::*;

pub fn main() {
    setup_global_error_handlers();
    mount_to_body(App());
}

#[component]
fn App() -> impl View {
    div((
        h1("Error Boundary Demo"),
        p("This example demonstrates how ErrorBoundary catches errors."),

        // 1. Recoverable Error (Result::Err) behavior
        div((
            h2("1. Recoverable Error Test"),
            ErrorBoundary(ErrorBoundaryProps {
                fallback: |err| {
                    div((
                        h3("Caught Recoverable Error!"),
                        p(format!("Error info: {}", err)),
                        button("Reset (Reload Page)").on_click(|_| {
                            let _ = web_sys::window().unwrap().location().reload();
                        })
                    ))
                    .style("background-color: #fee; border: 1px solid red; padding: 10px; color: red;")
                },
                children: || {
                    // 无参数组件直接调用，不需要传递 Props
                    RecoverableComponent()
                }
            }),
        )).style("margin-bottom: 20px; border: 1px solid #ccc; padding: 10px;"),

        // 2. Immediate Panic Test
        div((
            h2("2. Immediate Panic Test (Render Phase)"),
            p("Component below will panic completely upon rendering if triggered."),
            ErrorBoundary(ErrorBoundaryProps {
                fallback: |err| {
                    div((
                        h3("Caught Panic!"),
                        p(format!("Panic details: {}", err)),
                    ))
                    .style("background-color: #fff3cd; border: 1px solid orange; padding: 10px; color: #856404;")
                },
                children: || {
                    // 无参数组件直接调用
                    PanicToggleComponent()
                }
            }),
        )).style("margin-bottom: 20px; border: 1px solid #ccc; padding: 10px;"),
    )).style("padding: 20px; font-family: sans-serif;")
}

#[component]
fn RecoverableComponent() -> impl View {
    let (should_error, set_should_error) = signal(false);

    move || {
        if should_error.get() {
            // Return an Err, which triggers handle_error -> ErrorContext
            Err(SilexError::Javascript(
                "User clicked the error button!".into(),
            ))
        } else {
            Ok(div((
                p("Component is running normally."),
                button("Trigger Result::Err").on_click(move |_| {
                    set_should_error.set(true);
                }),
            )))
        }
    }
}

// A component that conditionally renders a child that panics immediately during construction
#[component]
fn PanicToggleComponent() -> impl View {
    let (show_panic, _set_show_panic) = signal(false);

    move || {
        if show_panic.get() {
            // We wrap the panicking component in a way that its construction is delayed until this closure runs
            // Because ErrorBoundary wraps this closure in effect and catch_unwind, it captures this panic.
            // 无参数组件直接调用
            Some(ImmediatePanic())
        } else {
            None
        }
    }
}

#[component]
fn ImmediatePanic() -> impl View {
    let (active, set_active) = signal(false);

    div((
        p("Ready to panic?"),
        button("Click to Panic Immediately").on_click(move |_| {
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
        },
    ))
}
