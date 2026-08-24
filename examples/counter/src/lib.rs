use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::dom::CleanupSink;
use silex::prelude::*;
use silex::reexports::*;

router! {
    pub enum AppRoute {
        Home => "/",
        About => "/about",
        NotFound => "/*",
    }
}

#[component]
fn Card<'scope, Ctx>(
    #[ctx] ctx: Ctx,
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
            sty(ctx)
                .margin_top(px(0))?
                .font_size(rem(1.2))?
                .color(hex("#333"))?
        ),
        child,
    )
    .class("card")
    .style(
        sty(ctx)
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
fn CounterDisplay<'scope, Ctx>(#[ctx] ctx: Ctx, count: Rx<'scope, i32>) -> impl View<'scope> {
    // Demo: Style Map (Vec) and Dynamic Class (Signal)
    let is_even = owner.computed(move || Ok(count.get()? % 2 == 0), error_handler)?;

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
            sty(ctx)
                .font_weight(FontWeightKeyword::Bold)?
                .color(hex("#6200ea"))?,
        ),
        div(" (Even Number - Dynamic Class Active)")
            .style(sty(ctx).margin_top(px(5))?)
            .style(rx!(ctx;
                format!(
                    "opacity: {}; transition: opacity 0.3s",
                    if *$is_even { 1.0 } else { 0.0 }
                )
            )?),
    )
    .class(container_class)
    .class(("even-number", rx!(ctx; *$is_even)?))) // Adds class "even-number" when count is even
}

#[component]
fn CounterControls<'scope, Ctx>(#[ctx] ctx: Ctx, count: Signal<'scope, i32>) -> impl View<'scope> {
    // Demo: Style Array
    let btn_style = sty(ctx)
        .padding("8px 16px")?
        .border_radius(px(4))?
        .border("1px solid #ccc")?
        .cursor(CursorKeyword::Pointer)?
        .background(ColorName::White)?
        .transition("background-color 0.2s")?;

    Ok(div!(
        button("-").style(btn_style.clone()).on_click(move |_| {
            count.update(|n| *n -= 1)?;
            Ok(())
        }),
        span(count).style(
            sty(ctx)
                .font_size(rem(1.5))?
                .font_weight(FontWeightKeyword::Bold)?
                .min_width(px(30))?
                .text_align(TextAlignKeyword::Center)?
        ),
        button("+").style(btn_style).on_click(move |_| {
            count.update(|n| *n += 1)?;
            Ok(())
        }),
    )
    .style(
        sty(ctx)
            .display("flex")?
            .align_items("center")?
            .gap(px(15))?,
    ))
}

// --- Views ---

#[component]
fn NavBar<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        Link(ctx, "/")
            .children("Home")
            .style(
                sty(ctx)
                    .margin_right(px(15))?
                    .text_decoration("none")?
                    .color(hex("#007bff"))?
                    .font_weight(FontWeightKeyword::Bold)?
            )
            .build(),
        Link(ctx, "/about")
            .children("About")
            .style(
                sty(ctx)
                    .text_decoration("none")?
                    .color(hex("#007bff"))?
                    .font_weight(FontWeightKeyword::Bold)?
            )
            .build(),
    )
    .style(
        sty(ctx)
            .margin_bottom(px(20))?
            .padding("10px")?
            .border_bottom("1px solid #eee")?,
    ))
}

#[component]
fn HomeView<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    let owner = ctx.owner();

    // 页面级状态
    let name = owner.signal("Rustacean".to_string())?;

    // 显式本地信号传递演示
    let count = owner.signal(0)?;
    let is_high = rx!(ctx; *$count > 5)?;
    let suspense_source = owner.constant(())?;
    let suspense_data_style = sty(ctx)
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
                .style(sty(ctx).color(hex("#666"))?),
        )
        .style(
            sty(ctx)
                .text_align(TextAlignKeyword::Center)?
                .margin_bottom(px(30))?
        ),
        // Card 1: Explicit Parameter Counter
        Card(ctx, "Explicit Counter")
            .elevation(3)
            .on_hover(owner.callback(|_| {
                web_sys::console::log_1(&"Card Hovered!".into());
                Ok(())
            })?)
            .child(chain!(
                CounterControls(ctx, count).build(),
                CounterDisplay(ctx, count).build(),
            ))
            .build(),
        // Card 2: Input & Local State
        Card(ctx, "Local State (Resets on Nav)")
            .child(div!(div!(
                div!(
                    span("Hello, "),
                    span(name).style(
                        sty(ctx)
                            .color(hex("#007bff"))?
                            .font_weight(FontWeightKeyword::Bold)?
                    ),
                    span("!"),
                )
                .style(sty(ctx).margin_bottom(px(10))?),
                input()
                    .type_("text")
                    .placeholder("Enter name")
                    .style(
                        sty(ctx)
                            .padding("8px")?
                            .border("1px solid #ccc")?
                            .border_radius(px(4))?
                            .width(pct(100))?
                    )
                    .value(name)
                    .on(event::input, move |event: DomEvent| {
                        name.set(event.input_value().unwrap_or_default())?;
                        Ok(())
                    })
            )))
            .build(),
        // Card 3: Control Flow
        Card(ctx, "Control Flow")
            .child(
                is_high
                    .when(
                        ctx,
                        div("⚠️ Warning: Count is getting high!").style(
                            sty(ctx)
                                .background("#ffebee")?
                                .color(hex("#c62828"))?
                                .padding("10px")?
                                .border_radius(px(4))?
                        )
                    )
                    .fallback(
                        div("✓ System works normally.").style(
                            sty(ctx)
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
        Card(ctx, "Suspense (Async Loading)")
            .child(
                Suspense(ctx, move |cx| {
                    match Resource::builder(owner)
                        .source(suspense_source)
                        .fetch(|_| async {
                            gloo_timers::future::TimeoutFuture::new(2_000).await;
                            Ok::<_, SilexError>("Loaded Data from Server!".to_string())
                        })
                        .suspense(cx)
                        .build(error_handler)
                    {
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
                            Ok(ErrorPage(ctx, error).build().into_any())
                        }
                    }
                })
                .fallback(
                    div("Loading data (approx 2s)...")
                        .style(sty(ctx).color(ColorName::Orange)?.font_style("italic")?)
                )
                .build(),
            )
            .build(),
    ))
}

#[component]
fn AboutView<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("About"),
        p("This is the About Page to demonstrate Silex Router."),
        p("Try going back to Home, and notice the Counter is preserved while being passed explicitly to components."),
    ).style(sty(ctx).padding("20px")?.text_align(TextAlignKeyword::Center)?).into_any())
}

#[component]
fn NotFound<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div(h1("404 - Page Not Found"))
        .style(sty(ctx).color(ColorName::Red)?.padding("20px")?)
        .into_any())
}

#[component]
fn ErrorPage<'scope, Ctx, Err>(#[ctx] ctx: Ctx, error: Err) -> impl View<'scope>
where
    Err: Into<SilexError>,
{
    Ok(div!(
        h1("Silex Application Error"),
        p(error.into().to_string()),
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
        sty(ctx)
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
    #[ctx] ctx: SilexContext<'scope>,
    root_error: Rx<'scope, Option<SilexError>>,
) -> impl View<'scope> {
    let app_style = sty(ctx)
        .font_family("-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif")?
        .max_width(px(600))?
        .margin("0 auto")?
        .padding("20px")?;

    let boundary = ErrorBoundary(ctx, move |boundary_ctx| {
        match AppRoute::table(|route, ctx| match route {
            AppRoute::Home => HomeView(ctx).build().into_any(),
            AppRoute::About => AboutView(ctx).build().into_any(),
            AppRoute::NotFound => NotFound(ctx).build().into_any(),
        }) {
            Ok(table) => div!(
                Router(boundary_ctx)
                    .routes(table)
                    .layout(move |ctx, outlet| { div!(NavBar(ctx).build(), outlet) })
                    .build()
            )
            .class("app-container")
            .style(app_style.clone())
            .into_any(),
            Err(error) => ErrorPage(
                boundary_ctx,
                SilexError::recoverable(SilexErrorKind::Framework(error.to_string())),
            )
            .build()
            .into_any(),
        }
    })
    .fallback(move |e| ErrorPage(ctx, e).build())
    .build()
    .into_any();

    Ok(rx!(ctx; {
        if let Some(error) = (*$root_error).clone() {
            ErrorPage(ctx, error).build().into_any()
        } else {
            boundary.clone()
        }
    })?)
}

/// Mount the counter application into the conventional `#app` target.
pub fn mount_counter() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    bootstrap.mount(Runtime::new(), mount_counter_view)?;
    bootstrap.into_js_host()
}

/// Mount the counter application into a caller-provided target node.
pub fn mount_counter_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_web_sys(target, CleanupSink::console())?;
    bootstrap.mount(Runtime::new(), mount_counter_view)?;
    bootstrap.into_js_host()
}

fn mount_counter_view<'scope>(ctx: &MountBuilderContext<'scope>) -> SilexResult<()> {
    let owner = ctx.access();
    let root_error = owner.signal(None::<SilexError>)?;
    let error_handler = owner.error_handler(move |error: SilexError| {
        let _ = root_error.set(Some(error));
    })?;
    let silex_ctx = SilexContext::new(owner, error_handler.view());
    let app = App(silex_ctx, root_error).build();
    ctx.mount_unit(app, error_handler)
}
