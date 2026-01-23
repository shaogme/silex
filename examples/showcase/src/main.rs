use silex::prelude::*;
use silex_macros::{Route, Store, component, css};

// ==================================================================================
// Phase 1: Basics - Components, Reactivity, Props, and Attributes
// ==================================================================================

mod basics {
    use super::*;

    #[component]
    pub fn Greeting(
        // Explicitly `into` is NOT needed for common types like String, PathBuf, Children, AnyView, and Callback.
        // The macro enables it by default, allowing you to pass string literals directly (e.g., .name("...")) without .into().
        // `default = "..."` specifies a fallback value if the prop is omitted.
        #[prop(default = "World")] name: String,
        #[prop(default)] punctuation: String,
    ) -> impl View {
        let full_punctuation = if punctuation.is_empty() {
            "!".to_string()
        } else {
            punctuation
        };

        div![
            span("Hello, "),
            strong(name).style("color: #007bff"),
            span(full_punctuation),
        ]
        .class("greeting-card")
        .style("padding: 10px; border: 1px solid #ddd; border-radius: 4px; margin-bottom: 10px;")
    }

    #[component]
    pub fn Counter() -> impl View {
        let (count, set_count) = create_signal(0);
        let double_count = create_memo(move || count.get() * 2);

        div![
            h3("Interactive Counter"),
            div![
                button("-").on_click(move |_| set_count.update(|n| *n -= 1)),
                strong(count),
                button("+").on_click(move |_| set_count.update(|n| *n += 1)),
            ]
            .style("display: flex; gap: 10px; align-items: center;"),
            div!["Double: ", double_count].style("margin-top: 5px; color: #666; font-size: 0.9em;"),
        ]
    }

    #[component]
    pub fn BasicsPage() -> impl View {
        div![
            h2("Basics"),
            Greeting().name("Developer"),
            Counter(),
            // AttributeDemo omitted for brevity, logic is same as previous
        ]
    }
}

// ==================================================================================
// Phase 2: Control Flow
// ==================================================================================

mod flow_control {
    use super::*;

    #[component]
    pub fn ListDemo() -> impl View {
        let (list, set_list) = create_signal(vec!["Apple", "Banana", "Cherry"]);

        div![
            h3("List Rendering with Signal Ergonomics"),
            p("Demonstrates passing a Signal directly to For::new without closure wrapper."),
            // BEFORE: For::new(move || list.get(), ...)
            // AFTER:  For::new(list, ...) - Accessor trait enables this!
            ul(For::new(list, |item| *item, |item| li(item))),
            button("Add Item").on_click(move |_| {
                set_list.update(|l| l.push("New Item"));
            }),
        ]
    }

    #[component]
    pub fn ShowDemo() -> impl View {
        let (visible, set_visible) = create_signal(true);

        div![
            h3("Conditional Rendering with Show"),
            p("Demonstrates passing a Signal directly to Show::new as condition."),
            button("Toggle Visibility").on_click(move |_| {
                set_visible.update(|v| *v = !*v);
            }),
            // BEFORE: Show::new(move || visible.get(), ...)
            // AFTER:  Show::new(visible, ...) - Accessor trait enables this!
            Show::new(
                visible,
                || div("✅ Content is visible!")
                    .style("color: green; padding: 10px; background: #e8f5e9;"),
                Some(|| div("❌ Content is hidden")
                    .style("color: red; padding: 10px; background: #ffebee;")),
            ),
        ]
    }

    #[component]
    pub fn DynamicDemo() -> impl View {
        let (mode, set_mode) = create_signal("A");

        div![
            h3("Dynamic Component Switching"),
            p("Demonstrates Dynamic component with closure accessor."),
            div![
                button("Show A").on_click(move |_| set_mode.set("A")),
                button("Show B").on_click(move |_| set_mode.set("B")),
                button("Show C").on_click(move |_| set_mode.set("C")),
            ]
            .style("display: flex; gap: 10px; margin-bottom: 10px;"),
            // Dynamic uses Accessor, so closures work seamlessly
            Dynamic::new(move || {
                view_match!(mode.get(), {
                    "A" => div("🅰️ Component A")
                        .style("padding: 20px; background: #e3f2fd;"),
                    "B" => div("🅱️ Component B")
                        .style("padding: 20px; background: #fff3e0;"),
                    _ => div("©️ Component C")
                        .style("padding: 20px; background: #f3e5f5;"),
                })
            }),
        ]
    }

    #[component]
    pub fn FlowPage() -> impl View {
        div![h2("Control Flow"), ListDemo(), ShowDemo(), DynamicDemo(),]
            .style("display: flex; flex-direction: column; gap: 20px;")
    }
}

// ==================================================================================
// Phase 3: Advanced (Store, CSS, Router)
// ==================================================================================

// --- Store Definition ---
#[derive(Clone, Default, Store)] // Macro generates UserSettingsStore
pub struct UserSettings {
    pub theme: String,
    pub notifications: bool,
    pub username: String,
}

mod advanced {
    use super::*;

    #[component]
    pub fn CssDemo() -> impl View {
        // Scoped CSS using css! macro
        let btn_class = css!(
            r#"
            background-color: #6200ea;
            color: white;
            padding: 8px 16px;
            border: none;
            border-radius: 4px;
            cursor: pointer;
            transition: transform 0.1s;

            &:hover {
                background-color: #3700b3;
                transform: scale(1.05);
            }

            &:active {
                transform: scale(0.95);
            }
        "#
        );

        div![
            h3("CSS-in-Rust Demo"),
            p("The button below is styled using the `css!` macro with scoped styles."),
            button("Scoped Style Button")
                .class(btn_class)
                .on_click(|_| console_log("Clicked!")),
        ]
    }

    #[component]
    pub fn StoreDemo() -> impl View {
        // Access global store provided in main
        // Note: `use_context::<T>()` is also available if you want to handle the Option manually.
        // `expect_context` is a convenience wrapper that panics if the context is missing.
        let settings = expect_context::<UserSettingsStore>();

        div![
            h3("Global Store Demo"),
            div![
                p![strong("Username: "), settings.username],
                p![strong("Theme: "), settings.theme],
                p![
                    strong("Notifications: "),
                    text(move || {
                        if settings.notifications.get() {
                            "On"
                        } else {
                            "Off"
                        }
                    }),
                ],
            ]
            .style("border: 1px solid #ccc; padding: 10px; margin-bottom: 10px;"),
            h4("Update Settings"),
            div![
                button("Toggle Theme").on_click(move |_| {
                    settings.theme.update(|t| {
                        *t = if t == "Light" {
                            "Dark".to_string()
                        } else {
                            "Light".to_string()
                        }
                    })
                }),
                button("Toggle Notifications")
                    .on_click(move |_| settings.notifications.update(|n| *n = !*n)),
                input()
                    .bind_value(settings.username)
                    .placeholder("Change username..."),
            ]
            .style("display: flex; gap: 10px;"),
        ]
    }
}

// --- Routing Definition ---

#[component]
fn SelectDemo() -> impl View {
    div("Select a demo above.")
}

#[derive(Route, Clone, PartialEq)]
enum AdvancedRoute {
    #[route("/", view = SelectDemoComponent)]
    Index,
    #[route("/css", view = advanced::CssDemoComponent)]
    Css,
    #[route("/store", view = advanced::StoreDemoComponent)]
    Store,
    #[route("/*", view = NotFoundPageComponent)]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/", view = HomePageComponent)]
    Home,
    #[route("/basics", view = basics::BasicsPageComponent)]
    Basics,
    #[route("/flow", view = flow_control::FlowPageComponent)]
    Flow,
    #[route("/advanced/*", view = AdvancedLayoutComponent)]
    Advanced {
        #[nested]
        route: AdvancedRoute,
    },
    #[route("/*", view = NotFoundPageComponent)]
    NotFound,
}

// --- Layout & App ---

#[component]
fn NavBar() -> impl View {
    let nav_link = css!(
        r#"
        color: white;
        text-decoration: none;
        padding: 8px 12px;
        border-radius: 4px;
        transition: background-color 0.2s;

        &:hover {
            background-color: rgba(255, 255, 255, 0.2);
        }

        &.active {
            background-color: #007bff;
            font-weight: bold;
        }
    "#
    );

    div![
        Link(AppRoute::Home).text("Home").class(&nav_link).active_class("active"),
        Link(AppRoute::Basics).text("Basics").class(&nav_link).active_class("active"),
        Link(AppRoute::Flow).text("Flow").class(&nav_link).active_class("active"),
        Link(AppRoute::Advanced {
            route: AdvancedRoute::Index,
        })
        .text("Advanced")
        .class(&nav_link)
        .active_class("active"),
    ]
    .style("background: #333; color: white; padding: 10px; margin-bottom: 20px; display: flex; gap: 15px; align-items: center;")
}

#[component]
fn AdvancedLayout(route: AdvancedRoute) -> impl View {
    div![
        h2("Advanced Features"),
        div![
            Link("/advanced/css").text("CSS Demo").class("tab"), // Support string literal
            Link(AppRoute::Advanced {
                route: AdvancedRoute::Store,
            })
            .text("Store Demo")
            .class("tab"),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 20px;"),
        // Delegate rendering to the route itself via RouteView
        route.render(),
    ]
}

#[component]
fn NotFoundPage() -> impl View {
    div("404 - Page Not Found").style("color: red; padding: 20px;")
}

#[component]
fn HomePage() -> impl View {
    div![
        h1("Welcome to Silex Showcase"),
        p("This example application demonstrates the core features of the Silex framework."),
        ul![
            li(Link(AppRoute::Basics).text("Basics: Components, Props, Signals")),
            li(Link(AppRoute::Flow).text("Flow Control: Loops, Conditions")),
            li(Link(AppRoute::Advanced {
                route: AdvancedRoute::Index,
            })
            .text("Advanced: Router to Store & CSS")),
        ],
    ]
}

fn main() {
    setup_global_error_handlers();

    // Global State Initialization
    let store = UserSettingsStore::new(UserSettings {
        theme: "Light".to_string(),
        notifications: true,
        username: "Guest".to_string(),
    });

    // Mount App
    mount_to_body(move || {
        // Provide Global Store to the entire app tree
        provide_context(store).unwrap();

        div![
            // Global Layout Shell
            NavBar(),
            // Root Router
            Router::new().match_route::<AppRoute>(),
        ]
    });
}
