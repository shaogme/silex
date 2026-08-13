use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

#[component]
fn Card<'scope>(
    scope: Scope<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[prop(into)] title: String,
    #[chain(default = 1)] elevation: u8,
    #[chain(default)] child: AnyView<'scope>,
    #[prop(into)]
    #[chain(default)]
    on_hover: Callback<'scope>,
) -> impl View<'scope> {
    let box_shadow = format!("0 4px {}px rgba(0,0,0,0.1)", elevation * 4);

    let mut root = div!(
        h1(title).style(
            sty()
                .margin_top(px(0))?
                .font_size(rem(1.2))?
                .color(hex("#333"))?
        ),
        child,
    )
    .class("card")
    .style(
        sty()
            .border("1px solid #e0e0e0")?
            .border_radius(px(8))?
            .padding("20px")?
            .margin_bottom(px(20))?
            .box_shadow(box_shadow)?
            .transition("transform 0.2s")?,
    );

    root = root.on_click(move |_| on_hover.invoke(()));

    Ok(root)
}

#[component]
fn CounterDisplay<'scope>(
    scope: Scope<'scope>,
    count: ReadSignal<'scope, i32>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    // Demo: Style Map (Vec) and Dynamic Class (Signal)
    let is_even = scope.memo(move |_| Ok(count.get()? % 2 == 0), error_handler)?;

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
        span(count).style(
            sty()
                .font_weight(FontWeightKeyword::Bold)?
                .color(hex("#6200ea"))?,
        ),
        div(" (Even Number - Dynamic Class Active)")
            .style(sty().margin_top(px(5))?)
            .style(rx!(scope; error_handler;
                format!(
                    "opacity: {}; transition: opacity 0.3s",
                    if *$is_even { 1.0 } else { 0.0 }
                )
            )),
    )
    .class(container_class)
    .class(("even-number", rx!(scope; error_handler; *$is_even)))) // Adds class "even-number" when count is even
}

#[component]
fn CounterControls<'scope>(
    count: ReadSignal<'scope, i32>,
    set_count: WriteSignal<'scope, i32>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    // Demo: Style Array
    let btn_style = sty()
        .padding("8px 16px")?
        .border_radius(px(4))?
        .border("1px solid #ccc")?
        .cursor(CursorKeyword::Pointer)?
        .background(ColorName::White)?
        .transition("background-color 0.2s")?;

    Ok(div!(
        button("-").style(btn_style.clone()).on_click(move |_| {
            set_count
                .update(|n| *n -= 1)
                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
            Ok(())
        }),
        span(count).style(
            sty()
                .font_size(rem(1.5))?
                .font_weight(FontWeightKeyword::Bold)?
                .min_width(px(30))?
                .text_align(TextAlignKeyword::Center)?
        ),
        button("+").style(btn_style).on_click(move |_| {
            set_count
                .update(|n| *n += 1)
                .map_err(|error| SilexError::fatal(SilexErrorKind::Reactivity(error)))?;
            Ok(())
        }),
    )
    .style(sty().display("flex")?.align_items("center")?.gap(px(15))?))
}

// --- Views ---

#[component]
fn NavBar<'scope>(
    ctx: RouterContext<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        Link(ctx, "/")
            .error_handler(error_handler)
            .children("Home")
            .style(
                sty()
                    .margin_right(px(15))?
                    .text_decoration("none")?
                    .color(hex("#007bff"))?
                    .font_weight(FontWeightKeyword::Bold)?
            )
            .build(),
        Link(ctx, "/about")
            .error_handler(error_handler)
            .children("About")
            .style(
                sty()
                    .text_decoration("none")?
                    .color(hex("#007bff"))?
                    .font_weight(FontWeightKeyword::Bold)?
            )
            .build(),
    )
    .style(
        sty()
            .margin_bottom(px(20))?
            .padding("10px")?
            .border_bottom("1px solid #eee")?,
    ))
}

#[component]
fn HomeView<'scope>(
    ctx: RouterContext<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();

    // 页面级状态
    let (name, set_name) = scope.signal("Rustacean".to_string())?;

    // 显式本地信号传递演示
    let (count, set_count) = scope.signal(0)?;
    let is_high = rx!(scope; error_handler; *$count > 5);
    let suspense_source = scope.constant(())?;
    let suspense_data_style = sty()
        .color(hex("#2e7d32"))?
        .font_weight(FontWeightKeyword::Bold)?
        .background("#e8f5e9")?
        .padding("10px")?
        .border_radius(px(4))?;

    Ok(div!(
        // Header
        div!(
            h1("Silex: Next Gen"),
            p("Builder Pattern + Router + Explicit State + Suspense")
                .style(sty().color(hex("#666"))?),
        )
        .style(
            sty()
                .text_align(TextAlignKeyword::Center)?
                .margin_bottom(px(30))?
        ),
        // Card 1: Explicit Parameter Counter
        Card(scope, "Explicit Counter")
            .error_handler(error_handler)
            .elevation(3)
            .on_hover(scope.callback(|_| {
                web_sys::console::log_1(&"Card Hovered!".into());
                Ok(())
            })?)
            .child(chain!(
                CounterControls(count, set_count)
                    .error_handler(error_handler)
                    .build(),
                CounterDisplay(scope, count)
                    .error_handler(error_handler)
                    .build(),
            ))
            .build(),
        // Card 2: Input & Local State
        Card(scope, "Local State (Resets on Nav)")
            .error_handler(error_handler)
            .child(div!(div!(
                div!(
                    span("Hello, "),
                    span(name).style(
                        sty()
                            .color(hex("#007bff"))?
                            .font_weight(FontWeightKeyword::Bold)?
                    ),
                    span("!"),
                )
                .style(sty().margin_bottom(px(10))?),
                input()
                    .type_("text")
                    .placeholder("Enter name")
                    .style(
                        sty()
                            .padding("8px")?
                            .border("1px solid #ccc")?
                            .border_radius(px(4))?
                            .width(pct(100))?
                    )
                    .value(name)
                    .on_input(move |val| {
                        set_name.set(val).map_err(|error| {
                            SilexError::fatal(SilexErrorKind::Reactivity(error))
                        })?;
                        Ok(())
                    })
            )))
            .build(),
        // Card 3: Control Flow
        Card(scope, "Control Flow")
            .error_handler(error_handler)
            .child(
                is_high
                    .when(
                        scope,
                        error_handler,
                        div("⚠️ Warning: Count is getting high!").style(
                            sty()
                                .background("#ffebee")?
                                .color(hex("#c62828"))?
                                .padding("10px")?
                                .border_radius(px(4))?
                        )
                    )
                    .fallback(
                        div("✓ System works normally.").style(
                            sty()
                                .background("#e8f5e9")?
                                .color(hex("#2e7d32"))?
                                .padding("10px")?
                                .border_radius(px(4))?
                        )
                    )
                    .build()
            )
            .build(),
        // Card 4: Suspense
        Card(scope, "Suspense (Async Loading)")
            .error_handler(error_handler)
            .child(
                Suspense(scope, error_handler, move |cx| {
                    match Resource::new(
                        scope,
                        suspense_source,
                        |_| async {
                            gloo_timers::future::TimeoutFuture::new(2_000).await;
                            Ok::<_, SilexError>("Loaded Data from Server!".to_string())
                        },
                        Some(cx),
                        error_handler,
                    ) {
                        Ok(async_data_local) => {
                            Ok(div(move || match async_data_local.get_data() {
                                Ok(Some(data)) => data,
                                Ok(None) => "Waiting...".to_string(),
                                Err(error) => {
                                    let _ = error_handler.handle(error.clone());
                                    format!("Resource error: {error}")
                                }
                            })
                            .style(suspense_data_style.clone())
                            .into_any())
                        }
                        Err(error) => {
                            let _ = error_handler.handle(error.clone());
                            Ok(ErrorPage(error)
                                .error_handler(error_handler)
                                .build()
                                .into_any())
                        }
                    }
                })
                .fallback(
                    div("Loading data (approx 2s)...")
                        .style(sty().color(ColorName::Orange)?.font_style("italic")?)
                )
                .build(),
            )
            .build(),
    ))
}

#[component]
fn AboutView<'scope>(#[chain] error_handler: ErrorReporter<'scope>) -> AnyView<'scope> {
    Ok(div!(
        h1("About"),
        p("This is the About Page to demonstrate Silex Router."),
        p("Try going back to Home, and notice the Counter is preserved while being passed explicitly to components."),
    ).style(sty().padding("20px")?.text_align(TextAlignKeyword::Center)?).into_any())
}

#[component]
fn NotFound<'scope>(#[chain] error_handler: ErrorReporter<'scope>) -> AnyView<'scope> {
    Ok(div(h1("404 - Page Not Found"))
        .style(sty().color(ColorName::Red)?.padding("20px")?)
        .into_any())
}

#[component]
fn ErrorPage<'scope>(
    error: SilexError,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> AnyView<'scope> {
    Ok(div!(
        h1("Silex Application Error"),
        p(error.to_string()),
        button("Reload Application").on_click(|_| {
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
    .style(
        sty()
            .max_width(px(600))?
            .margin("80px auto")?
            .padding("32px")?
            .font_family("sans-serif")?
            .border("1px solid #f0b4b4")?
            .background("#fff7f7")?
            .color(hex("#7f1d1d"))?,
    )
    .into_any())
}

#[component]
fn App<'scope>(
    scope: Scope<'scope>,
    root_error: ReadSignal<'scope, Option<SilexError>>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let app_style = sty()
        .font_family("-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif")?
        .max_width(px(600))?
        .margin("0 auto")?
        .padding("20px")?;

    let boundary = ErrorBoundary(scope, move |boundary_error_handler| {
        match routes!(AppRoutes {
            home "/" => move |ctx| {
                HomeView(ctx)
                    .error_handler(boundary_error_handler)
                    .build()
            },
            about "/about" => move |_ctx| AboutView().error_handler(boundary_error_handler).build(),
            not_found "/*" => move |_ctx| NotFound().error_handler(boundary_error_handler).build(),
        }) {
            Ok(routes) => div!(
                Router(scope, boundary_error_handler)
                    .routes(routes.table())
                    .layout(move |ctx, outlet| {
                        div!(
                            NavBar(ctx).error_handler(boundary_error_handler).build(),
                            outlet
                        )
                    })
                    .build()
            )
            .class("app-container")
            .style(app_style.clone())
            .into_any(),
            Err(error) => ErrorPage(SilexError::recoverable(SilexErrorKind::Framework(
                error.to_string(),
            )))
            .error_handler(boundary_error_handler)
            .build()
            .into_any(),
        }
    })
    .error_handler(error_handler)
    .fallback(move |e| ErrorPage(e).error_handler(error_handler).build())
    .build()
    .into_any();

    Ok(rx!(scope; error_handler; {
        if let Some(error) = (*$root_error).clone() {
            ErrorPage(error)
                .error_handler(error_handler)
                .build()
                .into_any()
        } else {
            boundary.clone()
        }
    }))
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
    let (root_error, set_root_error) = scope.signal(None::<SilexError>)?;
    let error_handler = scope.error_handler(move |error: SilexError| {
        let _ = set_root_error.set(Some(error));
    })?;
    let app = App(scope, root_error).error_handler(error_handler).build();
    context.mount(app, error_handler)
}
