use silex::dom::tag::*;
use silex::prelude::*;

#[component]
fn Card(
    #[prop(default = "Default Title".to_string(), into)] title: String,
    #[prop(default = 1)] elevation: u8,
    #[prop(default)] child: Children, // Defaults to empty AnyView
    #[prop(default, into)] on_hover: Callback,
) -> impl View {
    let style = format!(
        "border: 1px solid #e0e0e0; border-radius: 8px; padding: 20px; margin-bottom: 20px; box-shadow: 0 4px {}px rgba(0,0,0,0.1); transition: transform 0.2s;",
        elevation * 4
    );

    let mut root = div().class("card").style(&style);

    root = root.on_click(move |_| on_hover.call(()));

    root.child((
        h1().style("margin-top: 0; font-size: 1.2rem; color: #333;")
            .text(title),
        child,
    ))
}

#[component]
fn CounterDisplay() -> SilexResult<impl View> {
    let count = expect_context::<ReadSignal<i32>>();

    // Demo: Style Map (Vec) and Dynamic Class (Signal)
    let is_even = create_memo(move || count.get() % 2 == 0);

    let container_styles = vec![
        ("margin-top", "10px"),
        ("color", "#555"),
        ("font-size", "0.9rem"),
        ("padding", "15px"),
        ("border", "1px dashed #bbb"),
        ("background-color", "#fafafa"),
    ];

    Ok(div()
        .style(container_styles)
        .class(("even-number", is_even)) // Adds class "even-number" when count is even
        .child((
            span().text("Global Context Status: "),
            span()
                .style(("font-weight", "bold")) // Single tuple style
                .style(("color", "#6200ea"))
                .text(count),
            div()
                .style(("margin-top", "5px"))
                .style(move || format!("opacity: {}; transition: opacity 0.3s", if is_even.get() { 1.0 } else { 0.0 }))
                .text(" (Even Number - Dynamic Class Active)"),
        )))
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

    Ok(div().style("display: flex; align-items: center; gap: 15px;").child((
        button()
            .style(btn_style) // Apply array of styles
            .text("-")
            .on_click(move |_| { let _ = set_count.update(|n| *n -= 1); }),
        span()
            .style("font-size: 1.5rem; font-weight: bold; min-width: 30px; text-align: center;")
            .text(count),
        button()
            .style(btn_style)
            .text("+")
            .on_click(move |_| { let _ = set_count.update(|n| *n += 1); }),
    )))
}

// --- Views ---

#[component]
fn NavBar() -> impl View {
    div().style("margin-bottom: 20px; padding: 10px; border-bottom: 1px solid #eee").child((
        link("/").text("Home").style("margin-right: 15px; text-decoration: none; color: #007bff; font-weight: bold;"),
        link("/about").text("About").style("text-decoration: none; color: #007bff; font-weight: bold;")
    ))
}

#[component]
fn HomeView() -> impl View {
    // 页面级状态
    let (name, set_name) = create_signal("Rustacean".to_string());
    
    // 全局状态通过 Context 获取
    let count = expect_context::<ReadSignal<i32>>();
    
    let is_high = create_memo(move || count.get() > 5);

    // Async Resource
    let async_data: Resource<String, silex::SilexError> = create_resource(
        || (),
        |_| async {
            gloo_timers::future::TimeoutFuture::new(2_000).await;
            Ok("Loaded Data from Server!".to_string())
        }
    ).expect("Failed to create resource");

    div()
        .child((
            // Header
            div().style("text-align: center; margin-bottom: 30px;").child((
                h1().text("Silex: Next Gen"),
                p().style("color: #666").text("Builder Pattern + Router + Context + Suspense"),
            )),

            // Card 1: Context-Aware Counter
            Card::new()
                .title("Global Counter (Persists across Nav)")
                .elevation(3)
                .on_hover(|_| { let _ = web_sys::console::log_1(&"Card Hovered!".into()); })
                .child((
                    CounterControls::new(),
                    CounterDisplay::new(),
                )),

            // Card 2: Input & Local State
            Card::new()
                .title("Local State (Resets on Nav)")
                .child(div().child((
                    div().style("margin-bottom: 10px").child((
                        span().text("Hello, "),
                        span().style("color: #007bff; font-weight: bold;").text(name),
                        span().text("!"),
                    )),
                    input()
                        .type_("text")
                        .placeholder("Enter name")
                        .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; width: 100%;")
                        .value(name)
                        .on_input(move |val| { let _ = set_name.set(val); })
                ))),

            // Card 3: Control Flow
            Card::new()
                .title("Control Flow")
                .child(
                    is_high
                        .when(|| div()
                            .style("background: #ffebee; color: #c62828; padding: 10px; border-radius: 4px;")
                            .text("⚠️ Warning: Count is getting high!"))
                        .otherwise(|| div()
                            .style("background: #e8f5e9; color: #2e7d32; padding: 10px; border-radius: 4px;")
                            .text("✓ System works normally."))
                ),
            
            // Card 4: Suspense
            Card::new()
                .title("Suspense (Async Loading)")
                .child(
                        suspense()
                        .fallback(|| div().style("color: orange; font-style: italic;").text("Loading data (approx 2s)..."))
                        .children(move || {
                            div()
                                .style("color: #2e7d32; font-weight: bold; background: #e8f5e9; padding: 10px; border-radius: 4px;")
                                .text(move || async_data.get().unwrap_or("Waiting...".to_string()))
                        })
                )
        ))
}

#[component]
fn AboutView() -> impl View {
    div().style("padding: 20px; text-align: center;").child((
        h1().text("About"),
        p().text("This is the About Page to demonstrate Silex Router."),
        p().text("Try going back to Home, and notice the Global Counter is preserved (Context), while the Name input is reset (Local State)."),
    ))
}

#[component]
fn NotFound() -> impl View {
    div().style("color: red; padding: 20px;").child(
        h1().text("404 - Page Not Found")
    )
}

#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/")]
    Home,
    #[route("/about")]
    About,
    #[route("/*")]
    NotFound,
}

// --- Main ---

fn main() -> () {
    silex::dom::setup_global_error_handlers();
    let window = web_sys::window().expect("No Window");
    let document = window.document().expect("No Document");
    let app_container = document.get_element_by_id("app").expect("No App Element");

    create_scope(move || {
        // 全局状态 (App Store)
        let (count, set_count) = create_signal(0);
        
        // 注入全局 Context
        provide_context(count).expect("provide count failed");
        provide_context(set_count).expect("provide set_count failed");

        // 构建应用壳 (App Shell)
        let app = div()
            .class("app-container")
            .style("font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;")
            .child((
                NavBar::new(),
                Router::new()
                    .match_enum(|route: AppRoute| {
                        match route {
                            AppRoute::Home => HomeView::new().into_any(),
                            AppRoute::About => AboutView::new().into_any(),
                            AppRoute::NotFound => NotFound::new().into_any(),
                        }
                    })
            ));

        app.mount(&app_container);
    });
}
