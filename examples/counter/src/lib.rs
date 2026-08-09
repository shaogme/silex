use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[component]
fn Card<'scope>(
    scope: Scope<'scope>,
    #[prop(into)] title: String,
    #[chain(default = 1)] elevation: u8,
    #[chain(default)] child: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    on_hover: Callback<'scope>,
) -> impl View<'scope> {
    let style = format!(
        "border: 1px solid #e0e0e0; border-radius: 8px; padding: 20px; margin-bottom: 20px; box-shadow: 0 4px {}px rgba(0,0,0,0.1); transition: transform 0.2s;",
        elevation * 4
    );

    let mut root = div!(
        h1(title).style("margin-top: 0; font-size: 1.2rem; color: #333;"),
        child,
    )
    .class("card")
    .style(&style);

    root = root.on_click(move |_| on_hover.invoke(()));

    root
}

#[component]
fn CounterDisplay<'scope>(
    scope: Scope<'scope>,
    count: ReadSignal<'scope, i32>,
) -> SilexResult<impl View<'scope>> {
    // Demo: Style Map (Vec) and Dynamic Class (Signal)
    let is_even = scope.memo(move |_| count.get() % 2 == 0);

    // Demo: CSS-in-Rust (Scoped CSS)
    let container_class = css! {
        margin-top: 10px;
        color: #555;
        font-size: 0.9rem;
        padding: 15px;
        border: 1px dashed #bbb;
        background-color: #fafafa;
        transition: all 0.2s ease-in-out;

        &:hover {
            background-color: #f0f0f0;
            border-color: #999;
            transform: scale(1.01);
            box-shadow: 0 2px 8px rgba(0,0,0,0.05);
        }
    };

    Ok(div!(
        span("Global Status: "),
        span(count)
            .style(("font-weight", "bold")) // Single tuple style
            .style(("color", "#6200ea")),
        div(" (Even Number - Dynamic Class Active)")
            .style(("margin-top", "5px"))
            .style(rx!(scope;
                format!(
                    "opacity: {}; transition: opacity 0.3s",
                    if *$is_even { 1.0 } else { 0.0 }
                )
            )),
    )
    .class(container_class)
    .class(("even-number", rx!(scope; *$is_even)))) // Adds class "even-number" when count is even
}

#[component]
fn CounterControls<'scope>(
    count: ReadSignal<'scope, i32>,
    set_count: WriteSignal<'scope, i32>,
) -> SilexResult<impl View<'scope>> {
    // Demo: Style Array
    let btn_style = [
        ("padding", "8px 16px"),
        ("border-radius", "4px"),
        ("border", "1px solid #ccc"),
        ("cursor", "pointer"),
        ("background-color", "white"),
        ("transition", "background-color 0.2s"),
    ];

    Ok(div!(
        button("-")
            .style(btn_style) // Apply array of styles
            .on_click(move |_| {
                set_count.update(|n| *n -= 1);
                Ok(())
            }),
        span(count)
            .style("font-size: 1.5rem; font-weight: bold; min-width: 30px; text-align: center;"),
        button("+").style(btn_style).on_click(move |_| {
            set_count.update(|n| *n += 1);
            Ok(())
        }),
    )
    .style("display: flex; align-items: center; gap: 15px;"))
}

// --- Views ---

#[component]
fn NavBar<'scope>(ctx: RouterContext<'scope>) -> impl View<'scope> {
    div!(
        Link(ctx, "/")
            .children("Home")
            .style("margin-right: 15px; text-decoration: none; color: #007bff; font-weight: bold;")
            .build(),
        Link(ctx, "/about")
            .children("About")
            .style("text-decoration: none; color: #007bff; font-weight: bold;")
            .build(),
    )
    .style("margin-bottom: 20px; padding: 10px; border-bottom: 1px solid #eee")
}

#[component]
fn HomeView<'scope>(
    ctx: RouterContext<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();

    // 页面级状态
    let (name, set_name) = scope.signal("Rustacean".to_string());

    // 显式本地信号传递演示
    let (count, set_count) = scope.signal(0);
    let is_high = scope.memo(move |_| count.get() > 5);

    Ok(div!(
        // Header
        div!(
            h1("Silex: Next Gen"),
            p("Builder Pattern + Router + Explicit State + Suspense").style("color: #666"),
        )
        .style("text-align: center; margin-bottom: 30px;"),
        // Card 1: Explicit Parameter Counter
        Card(scope, "Explicit Counter")
            .elevation(3)
            .on_hover(scope.callback(|_| {
                web_sys::console::log_1(&"Card Hovered!".into());
                Ok(())
            })?)
            .child(chain!(
                CounterControls(count, set_count).build(),
                CounterDisplay(scope, count).build(),
            ))
            .build(),
        // Card 2: Input & Local State
        Card(scope, "Local State (Resets on Nav)").child(div!(div!(
            div!(
                span("Hello, "),
                span(name).style("color: #007bff; font-weight: bold;"),
                span("!"),
            )
            .style("margin-bottom: 10px"),
            input()
                .type_("text")
                .placeholder("Enter name")
                .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; width: 100%;")
                .value(name)
                .on_input(move |val| {
                    set_name.set(val);
                    Ok(())
                })
        )))
        .build(),
        // Card 3: Control Flow
        Card(scope, "Control Flow").child(
            is_high
                .when(
                    scope,
                    div("⚠️ Warning: Count is getting high!").style(
                        "background: #ffebee; color: #c62828; padding: 10px; border-radius: 4px;"
                    )
                )
                .fallback(div("✓ System works normally.").style(
                    "background: #e8f5e9; color: #2e7d32; padding: 10px; border-radius: 4px;"
                ))
                .build()
        )
        .build(),
        // Card 4: Suspense
        Card(scope, "Suspense (Async Loading)").child(
            Suspense(scope, move |cx| {
                let async_data_local = Resource::new(
                    scope,
                    scope.constant(()),
                    |_| async {
                        gloo_timers::future::TimeoutFuture::new(2_000).await;
                        Ok::<_, SilexError>("Loaded Data from Server!".to_string())
                    },
                    Some(cx),
                    error_handler,
                );
                div(rx!(scope; $async_data_local.clone().unwrap_or("Waiting...".to_string())))
                    .style("color: #2e7d32; font-weight: bold; background: #e8f5e9; padding: 10px; border-radius: 4px;")
            })
            .fallback(div("Loading data (approx 2s)...").style("color: orange; font-style: italic;"))
            .build(),
        )
        .build(),
    ))
}

#[component]
fn AboutView<'scope>() -> AnyView<'scope> {
    div!(
        h1("About"),
        p("This is the About Page to demonstrate Silex Router."),
        p("Try going back to Home, and notice the Counter is preserved while being passed explicitly to components."),
    ).style("padding: 20px; text-align: center;").into_any()
}

#[component]
fn NotFound<'scope>() -> AnyView<'scope> {
    div(h1("404 - Page Not Found"))
        .style("color: red; padding: 20px;")
        .into_any()
}

#[component]
fn ErrorPage<'scope>(error: SilexError) -> AnyView<'scope> {
    div!(
        h1("Silex Application Error"),
        p(error.to_string()),
        button("Reload Application").on_click(|_| {
            let window = web_sys::window()
                .ok_or_else(|| SilexError::Javascript("Window is unavailable".to_string()))?;
            window.location().reload().map_err(SilexError::from)?;
            Ok(())
        }),
    )
    .style("max-width: 600px; margin: 80px auto; padding: 32px; font-family: sans-serif; border: 1px solid #f0b4b4; background: #fff7f7; color: #7f1d1d;")
    .into_any()
}

#[component]
fn App<'scope>(
    scope: Scope<'scope>,
    root_error: ReadSignal<'scope, Option<SilexError>>,
) -> impl View<'scope> {
    let boundary = ErrorBoundary(
        scope,
        move |error_handler| {
            let routes = routes!(AppRoutes {
                home "/" => move |ctx| {
                     HomeView(ctx).error_handler(error_handler).build()
                },
                about "/about" => move |_ctx| AboutView().build(),
                not_found "/*" => move |_ctx| NotFound().build(),
            });
                div!(
                    Router(scope)
                        .routes(routes.table())
                        .layout(move |ctx, outlet| div!(NavBar(ctx).build(), outlet))
                        .build()
                )
            .class("app-container")
            .style("font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;")
        },
    )
    .fallback(|e| ErrorPage(e).build())
    .build()
    .into_any();

    rx!(scope; {
        if let Some(error) = (*$root_error).clone() {
            ErrorPage(error).build().into_any()
        } else {
            boundary.clone()
        }
    })
}

/// Mount the counter application into the conventional `#app` target.
pub fn mount_counter() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app")?;
    bootstrap.mount(Runtime::new(), mount_counter_view)?;
    bootstrap.into_js_host()
}

/// Mount the counter application into a caller-provided target node.
pub fn mount_counter_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target);
    bootstrap.mount(Runtime::new(), mount_counter_view)?;
    bootstrap.into_js_host()
}

fn mount_counter_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let (root_error, set_root_error) = scope.signal(None::<SilexError>);
    let error_handler = scope.error_handler(move |error: SilexError| {
        let _ = set_root_error.try_set(Some(error));
    });
    let app = App(scope, root_error).build();
    context.mount(app, error_handler)
}
