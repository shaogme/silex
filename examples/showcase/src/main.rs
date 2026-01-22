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
        #[prop(default = "World".to_string(), into)] name: String,
        #[prop(default, into)] punctuation: String,
    ) -> impl View {
        let full_punctuation = if punctuation.is_empty() {
            "!".to_string()
        } else {
            punctuation
        };

        div()
            .class("greeting-card")
            .style(
                "padding: 10px; border: 1px solid #ddd; border-radius: 4px; margin-bottom: 10px;",
            )
            .child((
                span().text("Hello, "),
                strong().style("color: #007bff").text(name),
                span().text(full_punctuation),
            ))
    }

    #[component]
    pub fn Counter() -> impl View {
        let (count, set_count) = create_signal(0);
        let double_count = create_memo(move || count.get() * 2);

        div().child((
            h3().text("Interactive Counter"),
            div()
                .style("display: flex; gap: 10px; align-items: center;")
                .child((
                    button()
                        .text("-")
                        .on_click(move |_| set_count.update(|n| *n -= 1)),
                    strong().text(count),
                    button()
                        .text("+")
                        .on_click(move |_| set_count.update(|n| *n += 1)),
                )),
            div()
                .style("margin-top: 5px; color: #666; font-size: 0.9em;")
                .child((text("Double: "), text(double_count))),
        ))
    }

    #[component]
    pub fn BasicsPage() -> impl View {
        div().child((
            h2().text("Basics"),
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

        div().child((
            h3().text("List Rendering"),
            ul().child(For::new(
                move || list.get(),
                |item| *item,
                |item| li().text(item),
            )),
        ))
    }

    #[component]
    pub fn FlowPage() -> impl View {
        div().child((h2().text("Control Flow"), ListDemo::new()))
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

        div().child((
            h3().text("CSS-in-Rust Demo"),
            p().text("The button below is styled using the `css!` macro with scoped styles."),
            button()
                .class(btn_class)
                .text("Scoped Style Button")
                .on_click(|_| silex::logging::console_log("Clicked!")),
        ))
    }

    #[component]
    pub fn StoreDemo() -> impl View {
        // Access global store provided in main
        let settings = expect_context::<UserSettingsStore>();

        div().child((
            h3().text("Global Store Demo"),
            div()
                .style("border: 1px solid #ccc; padding: 10px; margin-bottom: 10px;")
                .child((
                    p().child((
                        strong().text("Username: "),
                        text(settings.username.read_signal()),
                    )),
                    p().child((strong().text("Theme: "), text(settings.theme.read_signal()))),
                    p().child((
                        strong().text("Notifications: "),
                        text(move || {
                            if settings.notifications.get() {
                                "On"
                            } else {
                                "Off"
                            }
                        }),
                    )),
                )),
            h4().text("Update Settings"),
            div().style("display: flex; gap: 10px;").child((
                button().text("Toggle Theme").on_click(move |_| {
                    settings.theme.update(|t| {
                        *t = if t == "Light" {
                            "Dark".to_string()
                        } else {
                            "Light".to_string()
                        }
                    })
                }),
                button()
                    .text("Toggle Notifications")
                    .on_click(move |_| settings.notifications.update(|n| *n = !*n)),
                input()
                    .value(settings.username.read_signal())
                    .on_input(move |val| settings.username.set(val))
                    .placeholder("Change username..."),
            )),
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
    div()
        .style("background: #333; color: white; padding: 10px; margin-bottom: 20px; display: flex; gap: 15px;")
        .child((
            link("/").text("Home").style("color: white; text-decoration: none;"),
            link("/basics").text("Basics").style("color: white; text-decoration: none;"),
            link("/flow").text("Flow").style("color: white; text-decoration: none;"),
            link("/advanced").text("Advanced").style("color: white; text-decoration: none;"),
        ))
}

#[component]
fn AdvancedLayout(route: AdvancedRoute) -> impl View {
    div().child((
        h2().text("Advanced Features"),
        div()
            .style("display: flex; gap: 10px; margin-bottom: 20px;")
            .child((
                link("/advanced/css").text("CSS Demo").class("tab"),
                link("/advanced/store").text("Store Demo").class("tab"),
            )),
        // Direct match on the passed route enum
        // This avoids re-parsing the URL via an internal Router
        view_match!(route, {
            AdvancedRoute::Index => div().text("Select a demo above."),
            AdvancedRoute::Css => advanced::CssDemo::new(),
            AdvancedRoute::Store => advanced::StoreDemo::new(),
            AdvancedRoute::NotFound => div().text("Advanced Demo Not Found"),
        }),
    ))
}

#[component]
fn NotFoundPage() -> impl View {
    div()
        .style("color: red; padding: 20px;")
        .text("404 - Page Not Found")
}

#[component]
fn HomePage() -> impl View {
    div().child((
        h1().text("Welcome to Silex Showcase"),
        p().text("This example application demonstrates the core features of the Silex framework."),
        ul().child((
            li().child(link("/basics").text("Basics: Components, Props, Signals")),
            li().child(link("/flow").text("Flow Control: Loops, Conditions")),
            li().child(link("/advanced").text("Advanced: Router to Store & CSS")),
        )),
    ))
}

fn main() {
    silex::dom::setup_global_error_handlers();
    console_error_panic_hook::set_once();

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

        div().child((
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
