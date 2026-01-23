use silex::prelude::*;
use silex::reexports::*;

#[component]
fn Card(
    #[prop(default = "Default Title", into)] title: String,
    #[prop(default = 1)] elevation: u8,
    #[prop(default)] child: Children, // Defaults to empty AnyView
    #[prop(default, into)] on_hover: Callback,
) -> impl View {
    let style = format!(
        "border: 1px solid #e0e0e0; border-radius: 8px; padding: 20px; margin-bottom: 20px; box-shadow: 0 4px {}px rgba(0,0,0,0.1); transition: transform 0.2s;",
        elevation * 4
    );

    let mut root = div((
        h1(title).style("margin-top: 0; font-size: 1.2rem; color: #333;"),
        child,
    ))
    .class("card")
    .style(&style);

    root = root.on_click(move |_| on_hover.call(()));

    root
}

#[component]
fn CounterDisplay() -> SilexResult<impl View> {
    let count = expect_context::<ReadSignal<i32>>();

    // Demo: Style Map (Vec) and Dynamic Class (Signal)
    let is_even = memo(move || count.get() % 2 == 0);

    // Demo: CSS-in-Rust (Scoped CSS)
    let container_class = css!(r#"
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
    "#);

    Ok(div((
        span("Global Context Status: "),
        span(count)
            .style(("font-weight", "bold")) // Single tuple style
            .style(("color", "#6200ea")),
        div(" (Even Number - Dynamic Class Active)")
            .style(("margin-top", "5px"))
            .style(move || format!("opacity: {}; transition: opacity 0.3s", if is_even.get() { 1.0 } else { 0.0 })),
    ))
    .class(container_class)
    .class(("even-number", is_even))) // Adds class "even-number" when count is even
}

#[component]
fn CounterControls() -> SilexResult<impl View> {
    let set_count = expect_context::<WriteSignal<i32>>();
    let count = expect_context::<ReadSignal<i32>>();

    // Demo: Style Array
    let btn_style = [
        ("padding", "8px 16px"),
        ("border-radius", "4px"),
        ("border", "1px solid #ccc"),
        ("cursor", "pointer"),
        ("background-color", "white"),
        ("transition", "background-color 0.2s"),
    ];

    Ok(div((
        button("-")
            .style(btn_style) // Apply array of styles
            .on_click(move |_| { let _ = set_count.update(|n| *n -= 1); }),
        span(count)
            .style("font-size: 1.5rem; font-weight: bold; min-width: 30px; text-align: center;"),
        button("+")
            .style(btn_style)
            .on_click(move |_| { let _ = set_count.update(|n| *n += 1); }),
    )).style("display: flex; align-items: center; gap: 15px;"))
}

// --- Views ---

#[component]
fn NavBar() -> impl View {
    div((
        Link("/", "Home").style("margin-right: 15px; text-decoration: none; color: #007bff; font-weight: bold;"),
        Link("/about", "About").style("text-decoration: none; color: #007bff; font-weight: bold;")
    )).style("margin-bottom: 20px; padding: 10px; border-bottom: 1px solid #eee")
}

#[component]
fn HomeView() -> impl View {
    // 页面级状态
    let (name, set_name) = signal("Rustacean".to_string());
    
    // 全局状态通过 Context 获取
    let count = expect_context::<ReadSignal<i32>>();
    
    let is_high = memo(move || count.get() > 5);

    // Async Resource
    let async_data: Resource<String, silex::SilexError> = resource(
        || (),
        |_| async {
            gloo_timers::future::TimeoutFuture::new(2_000).await;
            Ok("Loaded Data from Server!".to_string())
        }
    ).expect("Failed to create resource");

    div((
        // Header
        div((
            h1("Silex: Next Gen"),
            p("Builder Pattern + Router + Context + Suspense").style("color: #666"),
        )).style("text-align: center; margin-bottom: 30px;"),

        // Card 1: Context-Aware Counter
        Card()
            .title("Global Counter (Persists across Nav)")
            .elevation(3)
            .on_hover(|_| { let _ = web_sys::console::log_1(&"Card Hovered!".into()); })
            .child((
                CounterControls(),
                CounterDisplay(),
            )),

        // Card 2: Input & Local State
        Card()
            .title("Local State (Resets on Nav)")
            .child(div(div((
                div((
                    span("Hello, "),
                    span(name).style("color: #007bff; font-weight: bold;"),
                    span("!"),
                )).style("margin-bottom: 10px"),
                input()
                    .type_("text")
                    .placeholder("Enter name")
                    .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; width: 100%;")
                    .value(name)
                    .on_input(move |val| { let _ = set_name.set(val); })
            )))),

        // Card 3: Control Flow
        Card()
            .title("Control Flow")
            .child(
                is_high
                    .when(|| div("⚠️ Warning: Count is getting high!")
                        .style("background: #ffebee; color: #c62828; padding: 10px; border-radius: 4px;"))
                    .fallback(|| div("✓ System works normally.")
                        .style("background: #e8f5e9; color: #2e7d32; padding: 10px; border-radius: 4px;"))
            ),
        
        // Card 4: Suspense
        Card()
            .title("Suspense (Async Loading)")
            .child(
                    suspense()
                    .fallback(|| div("Loading data (approx 2s)...").style("color: orange; font-style: italic;"))
                    .children(move || {
                        div(move || async_data.get().unwrap_or("Waiting...".to_string()))
                            .style("color: #2e7d32; font-weight: bold; background: #e8f5e9; padding: 10px; border-radius: 4px;")
                    })
            )
    ))
}

#[component]
fn AboutView() -> impl View {
    div((
        h1("About"),
        p("This is the About Page to demonstrate Silex Router."),
        p("Try going back to Home, and notice the Global Counter is preserved (Context), while the Name input is reset (Local State)."),
    )).style("padding: 20px; text-align: center;")
}

#[component]
fn NotFound() -> impl View {
    div(
        h1("404 - Page Not Found")
    ).style("color: red; padding: 20px;")
}

#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/", view = HomeView)]
    Home,
    #[route("/about", view = AboutView)]
    About,
    #[route("/*", view = NotFound)]
    NotFound,
}

// --- Main ---

fn main() -> () {
    setup_global_error_handlers();
    let window = web_sys::window().expect("No Window");
    let document = window.document().expect("No Document");
    let app_container = document.get_element_by_id("app").expect("No App Element");

    create_scope(move || {
        // 全局状态 (App Store)
        let (count, set_count) = signal(0);
        
        // 注入全局 Context
        provide_context(count);
        provide_context(set_count);

        // 构建应用壳 (App Shell)
        let app = div((
            NavBar(),
            Router::new()
                .match_route::<AppRoute>()
        ))
        .class("app-container")
        .style("font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;");

        app.mount(&app_container);
    });
}
