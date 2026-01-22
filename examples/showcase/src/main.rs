use silex::dom::tag::*;
use silex::prelude::*;
use silex_macros::{Route, Store, component, css};

// ==================================================================================
// Phase 1: Basics - Components, Reactivity, Props, and Attributes
// ==================================================================================

mod basics {
    use super::*;

    #[component]
    pub fn Greeting(
        #[prop(default = "World", into)] name: String,
        #[prop(default, into)] punctuation: String,
    ) -> impl View {
        let full_punctuation = if punctuation.is_empty() {
            "!".to_string()
        } else {
            punctuation
        };

        div((
            span("Hello, "),
            strong(name).style("color: #007bff"),
            span(full_punctuation),
        ))
        .class("greeting-card")
        .style("padding: 10px; border: 1px solid #ddd; border-radius: 4px; margin-bottom: 10px;")
    }

    #[component]
    pub fn Counter() -> impl View {
        let (count, set_count) = create_signal(0);
        let double_count = create_memo(move || count.get() * 2);

        div((
            h3("Interactive Counter"),
            div((
                button("-").on_click(move |_| set_count.update(|n| *n -= 1)),
                strong(count),
                button("+").on_click(move |_| set_count.update(|n| *n += 1)),
            ))
            .style("display: flex; gap: 10px; align-items: center;"),
            div(("Double: ", double_count))
                .style("margin-top: 5px; color: #666; font-size: 0.9em;"),
        ))
    }

    #[component]
    pub fn BasicsPage() -> impl View {
        div((
            h2("Basics"),
            Greeting::new().name("Developer"),
            Counter::new(),
            // AttributeDemo omitted for brevity, logic is same as previous
        ))
    }
}

// ==================================================================================
// Phase 2: Control Flow
// ==================================================================================

mod flow_control {
    use super::*;

    #[component]
    pub fn ListDemo() -> impl View {
        let (list, _set_list) = create_signal(vec!["Apple", "Banana", "Cherry"]);

        div((
            h3("List Rendering"),
            ul(For::new(move || list.get(), |item| *item, |item| li(item))),
        ))
    }

    #[component]
    pub fn FlowPage() -> impl View {
        div((h2("Control Flow"), ListDemo::new()))
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

        div((
            h3("CSS-in-Rust Demo"),
            p("The button below is styled using the `css!` macro with scoped styles."),
            button("Scoped Style Button")
                .class(btn_class)
                .on_click(|_| silex::logging::console_log("Clicked!")),
        ))
    }

    #[component]
    pub fn StoreDemo() -> impl View {
        // Access global store provided in main
        // Note: `use_context::<T>()` is also available if you want to handle the Option manually.
        // `expect_context` is a convenience wrapper that panics if the context is missing.
        let settings = expect_context::<UserSettingsStore>();

        div((
            h3("Global Store Demo"),
            div((
                p((strong("Username: "), settings.username)),
                p((strong("Theme: "), settings.theme)),
                p((
                    strong("Notifications: "),
                    text(move || {
                        if settings.notifications.get() {
                            "On"
                        } else {
                            "Off"
                        }
                    }),
                )),
            ))
            .style("border: 1px solid #ccc; padding: 10px; margin-bottom: 10px;"),
            h4("Update Settings"),
            div((
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
            ))
            .style("display: flex; gap: 10px;"),
        ))
    }
}

// --- Routing Definition ---

#[derive(Route, Clone, PartialEq)]
enum AdvancedRoute {
    #[route("/")]
    Index,
    #[route("/css")]
    Css,
    #[route("/store")]
    Store,
    #[route("/*")]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/")]
    Home,
    #[route("/basics")]
    Basics,
    #[route("/flow")]
    Flow,
    #[route("/advanced/*")]
    #[nested]
    Advanced(AdvancedRoute),
    #[route("/*")]
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

    div((
        Link("/").text("Home").class(&nav_link).active_class("active"),
        Link("/basics").text("Basics").class(&nav_link).active_class("active"),
        Link("/flow").text("Flow").class(&nav_link).active_class("active"),
        Link("/advanced").text("Advanced").class(&nav_link).active_class("active"),
    ))
    .style("background: #333; color: white; padding: 10px; margin-bottom: 20px; display: flex; gap: 15px; align-items: center;")
}

#[component]
fn AdvancedLayout(route: AdvancedRoute) -> impl View {
    div((
        h2("Advanced Features"),
        div((
            Link("/advanced/css").text("CSS Demo").class("tab"),
            Link("/advanced/store").text("Store Demo").class("tab"),
        ))
        .style("display: flex; gap: 10px; margin-bottom: 20px;"),
        // Direct match on the passed route enum
        // This avoids re-parsing the URL via an internal Router
        view_match!(route, {
            AdvancedRoute::Index => div("Select a demo above."),
            AdvancedRoute::Css => advanced::CssDemo::new(),
            AdvancedRoute::Store => advanced::StoreDemo::new(),
            AdvancedRoute::NotFound => div("Advanced Demo Not Found"),
        }),
    ))
}

#[component]
fn NotFoundPage() -> impl View {
    div("404 - Page Not Found").style("color: red; padding: 20px;")
}

#[component]
fn HomePage() -> impl View {
    div((
        h1("Welcome to Silex Showcase"),
        p("This example application demonstrates the core features of the Silex framework."),
        ul((
            li(Link("/basics").text("Basics: Components, Props, Signals")),
            li(Link("/flow").text("Flow Control: Loops, Conditions")),
            li(Link("/advanced").text("Advanced: Router to Store & CSS")),
        )),
    ))
}

fn main() {
    silex::dom::setup_global_error_handlers();

    // Global State Initialization
    let store = UserSettingsStore::new(UserSettings {
        theme: "Light".to_string(),
        notifications: true,
        username: "Guest".to_string(),
    });

    // Mount App
    silex::dom::mount_to_body(move || {
        // Provide Global Store to the entire app tree
        provide_context(store).unwrap();

        div((
            // Global Layout Shell
            NavBar::new(),
            // Root Router
            Router::new().match_enum(|route: AppRoute| {
                view_match!(route, {
                    AppRoute::Home => HomePage::new(),
                    AppRoute::Basics => basics::BasicsPage::new(),
                    AppRoute::Flow => flow_control::FlowPage::new(),

                    // Pass the nested enum to the sub-handler
                    // Now AdvancedLayout takes the route directly as a prop
                    AppRoute::Advanced(inner) => AdvancedLayout::new(inner),

                    AppRoute::NotFound => NotFoundPage::new(),
                })
            }),
        ))
    });
}
