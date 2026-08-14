use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[component]
fn App<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let recoverable_fallback_style = sty(ctx)
        .background_color(hex("#fee"))?
        .border("1px solid red")?
        .padding("10px")?
        .color(ColorName::Red)?;
    let panic_fallback_style = sty(ctx)
        .background_color(hex("#fff3cd"))?
        .border("1px solid orange")?
        .padding("10px")?
        .color(hex("#856404"))?;
    let section_style = sty(ctx)
        .margin_bottom(px(20))?
        .border("1px solid #ccc")?
        .padding("10px")?;
    let root_style = sty(ctx).padding("20px")?.font_family("sans-serif")?;

    Ok(div!(
        h1("Error Boundary Demo"),
        p("This example demonstrates how ErrorBoundary catches errors."),
        div!(
            h2("1. Recoverable Error Test"),
            ErrorBoundary(ctx, move |child_ctx| {
                RecoverableComponent(child_ctx).build()
            })
            .fallback(move |error| {
                div!(
                    h3("Caught Recoverable Error!"),
                    p(format!("Error info: {}", error)),
                    button("Reset (Reload Page)").on_click(|_| {
                        let window = web_sys::window().ok_or_else(|| {
                            SilexError::fatal(SilexErrorKind::Javascript(
                                "Window is unavailable".to_string(),
                            ))
                        })?;
                        window
                            .location()
                            .reload()
                            .map_err(|error| SilexError::fatal(SilexErrorKind::from(error)))?;
                        Ok(())
                    }),
                )
                .style(recoverable_fallback_style.clone())
            })
            .build(),
        )
        .style(section_style.clone()),
        div!(
            h2("2. Immediate Panic Test (Render Phase)"),
            p("The component below panics during rendering when triggered."),
            ErrorBoundary(ctx, move |child_ctx| {
                PanicToggleComponent(child_ctx).build()
            })
            .fallback(move |error| {
                div!(h3("Caught Panic!"), p(format!("Panic details: {}", error)),)
                    .style(panic_fallback_style.clone())
            })
            .build(),
        )
        .style(section_style),
    )
    .style(root_style))
}

#[component]
fn RecoverableComponent<'scope>(
    #[ctx] ctx: SilexContext<'scope>,
) -> impl View<'scope> + 'scope {
    let (should_error, set_should_error) = scope.signal(false)?;

    Ok(move || {
        if should_error.get()? {
            Err(SilexError::recoverable(SilexErrorKind::Framework(
                "User clicked the error button!".to_string(),
            )))
        } else {
            Ok(div!(
                p("Component is running normally."),
                button("Trigger Result::Err").on_click(move |_| {
                    set_should_error.set(true)?;
                    Ok(())
                }),
            ))
        }
    })
}

#[component]
fn PanicToggleComponent<'scope>(
    #[ctx] ctx: SilexContext<'scope>,
) -> impl View<'scope> + 'scope {
    let (show_panic, set_show_panic) = scope.signal(false)?;
    let immediate_panic = ImmediatePanic(ctx).build();

    Ok(move || {
        if show_panic.get()? {
            Ok(Some(immediate_panic.clone().into_any()))
        } else {
            Ok(Some(
                div!(
                    p("The panic component is currently hidden."),
                    button("Show Panic Component").on_click(move |_| {
                        set_show_panic.set(true)?;
                        Ok(())
                    }),
                )
                .into_any(),
            ))
        }
    })
}

#[component]
fn ImmediatePanic<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> + 'scope {
    let (active, set_active) = scope.signal(false)?;

    Ok(div!(
        p("Ready to panic?"),
        button("Click to Panic Immediately").on_click(move |_| {
            set_active.set(true)?;
            Ok(())
        }),
        move || -> SilexResult<String> {
            if active.get()? {
                panic!("KA-BOOM! Panic in render function.");
            }
            Ok("Safe".to_string())
        },
    ))
}

/// Mount the error boundary demo into the conventional `#app` target.
pub fn mount_error_boundary_demo() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    bootstrap.mount(Runtime::new(), mount_error_boundary_demo_view)?;
    bootstrap.into_js_host()
}

/// Mount the error boundary demo into a caller-provided target node.
pub fn mount_error_boundary_demo_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target, CleanupSink::console());
    bootstrap.mount(Runtime::new(), mount_error_boundary_demo_view)?;
    bootstrap.into_js_host()
}

fn mount_error_boundary_demo_view<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let scope = ctx.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    ctx.mount(
        App(SilexContext::new(scope, error_handler)).build(),
        error_handler,
    )
}
