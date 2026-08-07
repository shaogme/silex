use silex::dom::view::ScopedViewOwner;
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

    root = root.on_click(move |_| {
        let _ = on_hover.invoke(());
    });

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
            }),
        span(count)
            .style("font-size: 1.5rem; font-weight: bold; min-width: 30px; text-align: center;"),
        button("+").style(btn_style).on_click(move |_| {
            set_count.update(|n| *n += 1);
        }),
    )
    .style("display: flex; align-items: center; gap: 15px;"))
}

// --- Views ---

#[component]
fn NavBar<'scope>(scope: Scope<'scope>) -> impl View<'scope> {
    div!(
        Link(AppRoute::Home)
            .children("Home")
            .style("margin-right: 15px; text-decoration: none; color: #007bff; font-weight: bold;"),
        Link(AppRoute::About)
            .children("About")
            .style("text-decoration: none; color: #007bff; font-weight: bold;"),
    )
    .style("margin-bottom: 20px; padding: 10px; border-bottom: 1px solid #eee")
}

#[component]
fn HomeView<'scope>(ctx: RouterContext<'scope>) -> impl View<'scope> {
    let scope = ctx.scope();

    // 页面级状态
    let (name, set_name) = scope.signal("Rustacean".to_string());

    // 显式本地信号传递演示
    let (count, set_count) = scope.signal(0);
    let is_high = scope.memo(move |_| count.get() > 5);

    div!(
        // Header
        div!(
            h1("Silex: Next Gen"),
            p("Builder Pattern + Router + Explicit State + Suspense").style("color: #666"),
        ).style("text-align: center; margin-bottom: 30px;"),

        // Card 1: Explicit Parameter Counter
         Card(scope, "Explicit Counter")
             .elevation(3)
             .on_hover(scope.callback(|_| {
                 web_sys::console::log_1(&"Card Hovered!".into());
             }))
             .child(view_chain!(
                 CounterControls(count, set_count),
                 CounterDisplay(scope, count),
             )),

        // Card 2: Input & Local State
         Card(scope, "Local State (Resets on Nav)")
            .child(div!(div!(
                div!(
                    span("Hello, "),
                    span(name).style("color: #007bff; font-weight: bold;"),
                    span("!"),
                ).style("margin-bottom: 10px"),
                input()
                    .type_("text")
                    .placeholder("Enter name")
                    .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; width: 100%;")
                    .value(name)
                    .on_input(move |val| { set_name.set(val); })
            ))),

        // Card 3: Control Flow
         Card(scope, "Control Flow")
             .child(
                 is_high.when(scope, div("⚠️ Warning: Count is getting high!")
                         .style("background: #ffebee; color: #c62828; padding: 10px; border-radius: 4px;"))
                     .fallback(div("✓ System works normally.")
                        .style("background: #e8f5e9; color: #2e7d32; padding: 10px; border-radius: 4px;"))
            ),
        // Card 4: Suspense
         Card(scope, "Suspense (Async Loading)")
             .child(
                 Suspense(scope, move |cx| {
                     let async_data_local = Resource::new(
                         scope,
                         scope.constant(()),
                         |_| async {
                            gloo_timers::future::TimeoutFuture::new(2_000).await;
                            Ok::<_, SilexError>("Loaded Data from Server!".to_string())
                        },
                         Some(cx),
                     );
                     div(rx!(scope; $async_data_local.clone().unwrap_or("Waiting...".to_string())))
                        .style("color: #2e7d32; font-weight: bold; background: #e8f5e9; padding: 10px; border-radius: 4px;")
                })
                .fallback(div("Loading data (approx 2s)...").style("color: orange; font-style: italic;"))
            )
    )
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

#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/", view = HomeView, pass_ctx = true)]
    Home,
    #[route("/about", view = AboutView)]
    About,
    #[route("/*", view = NotFound)]
    NotFound,
}

// --- Main ---

fn main() {
    setup_global_error_handlers();
    let window = web_sys::window().expect("No Window");
    let document = window.document().expect("No Document");
    let app_container = document.get_element_by_id("app").expect("No App Element");

    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.with_scope(|scope| {
        // 构建应用壳 (App Shell)
        let app = div!(
            NavBar(scope),
            Router(scope).match_route::<AppRoute>()
        )
        .class("app-container")
        .style("font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;");

        let owner = ScopedViewOwner::new(scope);
        if let Err(error) = app.mount(&owner, app_container.as_ref(), Vec::new()) {
            ErrorReporter::unhandled().report(error);
        }
    });
}
