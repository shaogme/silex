use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[component]
fn App<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    div!(
        h1("Error Boundary Demo"),
        p("This example demonstrates how ErrorBoundary catches errors."),

        div!(
            h2("1. Recoverable Error Test"),
            ErrorBoundary(scope, move |_child_error_handler| {
                RecoverableComponent(scope).build()
            })
            .fallback(|error| {
                div!(
                    h3("Caught Recoverable Error!"),
                    p(format!("Error info: {}", error)),
                    button("Reset (Reload Page)").on_click(|_| {
                        let window = web_sys::window().ok_or_else(|| {
                            SilexError::Javascript("Window is unavailable".to_string())
                        })?;
                        window.location().reload().map_err(SilexError::from)?;
                        Ok(())
                    }),
                )
                .style(
                    "background-color: #fee; border: 1px solid red; padding: 10px; color: red;",
                )
            })
            .build(),
        )
        .style("margin-bottom: 20px; border: 1px solid #ccc; padding: 10px;"),

        div!(
            h2("2. Immediate Panic Test (Render Phase)"),
            p("The component below panics during rendering when triggered."),
            ErrorBoundary(scope, move |_child_error_handler| {
                PanicToggleComponent(scope).build()
            })
            .fallback(|error| {
                div!(
                    h3("Caught Panic!"),
                    p(format!("Panic details: {}", error)),
                )
                .style(
                    "background-color: #fff3cd; border: 1px solid orange; padding: 10px; color: #856404;",
                )
            })
            .build(),
        )
        .style("margin-bottom: 20px; border: 1px solid #ccc; padding: 10px;"),
    )
    .style("padding: 20px; font-family: sans-serif;")
}

#[component]
fn RecoverableComponent<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (should_error, set_should_error) = scope.signal(false);

    move || {
        if should_error.get() {
            Err(SilexError::Javascript(
                "User clicked the error button!".to_string(),
            ))
        } else {
            Ok(div!(
                p("Component is running normally."),
                button("Trigger Result::Err").on_click(move |_| {
                    set_should_error.set(true);
                    Ok(())
                }),
            ))
        }
    }
}

#[component]
fn PanicToggleComponent<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (show_panic, set_show_panic) = scope.signal(false);
    let immediate_panic = ImmediatePanic(scope);

    move || {
        if show_panic.get() {
            Some(immediate_panic.clone().build().into_any())
        } else {
            Some(
                div!(
                    p("The panic component is currently hidden."),
                    button("Show Panic Component").on_click(move |_| {
                        set_show_panic.set(true);
                        Ok(())
                    }),
                )
                .into_any(),
            )
        }
    }
}

#[component]
fn ImmediatePanic<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    let (active, set_active) = scope.signal(false);

    div!(
        p("Ready to panic?"),
        button("Click to Panic Immediately").on_click(move |_| {
            set_active.set(true);
            Ok(())
        }),
        move || {
            if active.get() {
                panic!("KA-BOOM! Panic in render function.");
            }
            "Safe"
        },
    )
}

/// Mount the error boundary demo into the conventional `#app` target.
pub fn mount_error_boundary_demo() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app")?;
    bootstrap.mount(Runtime::new(), mount_error_boundary_demo_view)?;
    bootstrap.into_js_host()
}

/// Mount the error boundary demo into a caller-provided target node.
pub fn mount_error_boundary_demo_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target);
    bootstrap.mount(Runtime::new(), mount_error_boundary_demo_view)?;
    bootstrap.into_js_host()
}

fn mount_error_boundary_demo_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    });
    context.mount(App(scope).build(), error_handler)
}
