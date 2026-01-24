use silex::prelude::*;
use std::time::Duration;

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
        #[prop(default = "World")] name: Signal<String>,
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
        .style(style! {
            padding: "10px",
            border: "1px solid #ddd",
            "border-radius": "4px",
            "margin-bottom": "10px"
        })
    }

    #[component]
    pub fn Counter() -> impl View {
        let (count, set_count) = signal(0);
        let double_count = Memo::new(move |_| count.get() * 2); // rx!(count.get() * 2) is move || count.get() * 2
        
        // Timer Handle for Auto Increment (StoredValue: doesn't trigger UI updates itself)
        let timer = StoredValue::new(None::<IntervalHandle>);
        // UI State for the timer
        let (is_running, set_is_running) = signal(false);

        div![
            h3("Interactive Counter"),
            div![
                button("-")
                    .attr("disabled", count.le(0)) // New: Directly pass Signal<bool> to attribute
                    .on(event::click, set_count.updater(|n| *n -= 1)),
                strong(count).classes(classes![
                    "counter-val",
                    "positive" => count.gt(0),
                    "negative" => count.lt(0)
                ]),
                button("+").on(event::click, set_count.updater(|n| *n += 1)),
            ]
            .style("display: flex; gap: 10px; align-items: center;"),
            
            // Auto Increment Demo using set_interval and StoredValue
            div![
                button(move || if is_running.get() { "Stop Auto Inc" } else { "Start Auto Inc" })
                    .on(event::click, move |_| {
                        if is_running.get() {
                            if let Some(handle) = timer.get_value() {
                                handle.clear();
                            }
                            timer.set_value(None);
                            set_is_running.set(false);
                        } else {
                            if let Ok(handle) = set_interval_with_handle(move || {
                                set_count.update(|n| *n += 1);
                            }, Duration::from_millis(1000)) {
                                timer.set_value(Some(handle));
                                set_is_running.set(true);
                            }
                        }
                    })
            ].style("margin: 10px 0;"),

            // Manual Input Demo using event_target_value
            div![
                span("Set Value: "),
                input()
                    .prop("value", count) // One-way binding from signal to DOM
                    .on(event::input, move |e| {
                        let val_str = event_target_value(&e);
                        if let Ok(n) = val_str.parse::<i32>() {
                            set_count.set(n);
                        }
                    })
            ].style("margin-bottom: 10px;"),

            div!["Double: ", double_count]
                .classes(rx!(if count.get() % 2 == 0 { "even" } else { "odd" }))
                .style("margin-top: 5px; color: #666; font-size: 0.9em;"),
        ]
    }

    #[component]
    pub fn NodeRefDemo() -> impl View {
        use silex::reexports::web_sys::HtmlInputElement;
        let input_ref = NodeRef::<HtmlInputElement>::new();

        div![
            h3("NodeRef Demo"),
            p("Click the button to focus the input field using direct DOM access."),
            input()
                .placeholder("I will be focused...")
                .node_ref(input_ref) // NodeRef 是 Copy 的，无需 clone
                .style("margin-right: 10px; padding: 5px;"),
            button("Focus Input").on(event::click, move |_| {
                if let Some(el) = input_ref.get() {
                    let _ = el.focus();
                }
            })
        ]
        .style("padding: 20px; border: 1px dashed #999; margin-top: 20px;")
    }
    #[component]
    pub fn SvgIconDemo() -> impl View {
        #[component]
        fn ShieldCheck() -> Element {
            svg(
                path()
                    .attr("stroke-linecap", "round")
                    .attr("stroke-linejoin", "round")
                    .attr("stroke-width", "2")
                    .attr("d", "M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"),
            )
            .attr("viewBox", "0 0 24 24")
            .attr("fill", "none")
            .attr("stroke", "currentColor")
            .attr("width", "24")
            .attr("height", "24")
        }

        div![
            h3("SVG Icon forwarding"),
            p("SVG icons with attribute forwarding."),
            div![
                ShieldCheck().style("width: 32px; height: 32px; color: green;"),
                ShieldCheck()
                    .style("width: 48px; height: 48px; color: blue; margin-left: 10px; cursor: pointer;")
                    .on(event::click, |_| console_log("Icon Clicked!")),
                ShieldCheck()
                    .attr("width", "50")
                    .attr("height", "50")
                    .style("color: red; margin-left: 10px;"),
            ]
            .style("display: flex; align-items: center; padding: 10px; background: white; border: 1px solid #ddd;")
        ]
        .style("margin-top: 20px;")
    }

    #[component]
    pub fn BasicsPage() -> impl View {
        let name_signal = RwSignal::new("Developer".to_string());

        div![
            h2("Basics"),
            div![
                "Reactive Greeting Name: ",
                "Reactive Greeting Name: ",
                input().bind_value(name_signal),
                button("Submit")
                    .attr("disabled", name_signal.read_signal().eq(""))
                    .style("margin-left: 10px;")
            ].style("margin-bottom: 15px; padding: 10px; background: #f8f9fa; border-radius: 4px;"),
            
            Greeting().name(name_signal),
            Counter(),
            NodeRefDemo(),
            SvgIconDemo(),
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
        let (list, set_list) = signal(vec!["Apple", "Banana", "Cherry"]);

        div![
            h3("List Rendering with Signal Ergonomics"),
            p("Demonstrates passing a Signal directly to For::new without closure wrapper."),
            ul(For::new(list, |item| *item, |item| li(item))),
            button("Add Item").on(event::click, set_list.updater(|l| l.push("New Item"))),
        ]
    }

    #[component]
    pub fn ShowDemo() -> impl View {
        let (visible, set_visible) = signal(true);

        div![
            h3("Conditional Rendering with Show"),
            p("Demonstrates passing a Signal directly to Show::new as condition."),
            button("Toggle Visibility").on(event::click, set_visible.updater(|v| *v = !*v)),
            Show::new(visible, || div("✅ Content is visible!")
                .style("color: green; padding: 10px; background: #e8f5e9;"),)
            .fallback(|| div("❌ Content is hidden")
                .style("color: red; padding: 10px; background: #ffebee;")),
        ]
    }

    #[component]
    pub fn DynamicDemo() -> impl View {
        let (mode, set_mode) = signal("A");

        div![
            h3("Dynamic Component Switching"),
            p("Demonstrates Dynamic component with closure accessor."),
            div![
                button("Show A").on(event::click, set_mode.setter("A")),
                button("Show B").on(event::click, set_mode.setter("B")),
                button("Show C").on(event::click, set_mode.setter("C")),
            ]
            .style("display: flex; gap: 10px; margin-bottom: 10px;"),
            // You can also use Dynamic::new(mode.map(|m| { view_match!(m, { ... }) })).
            Dynamic::bind(mode, |m| {
                view_match!(m, {
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
    pub fn SwitchDemo() -> impl View {
        let (tab, set_tab) = signal(0);

        div![
            h3("Switch (Match) Demo"),
            div![
                button("Tab 1").on(event::click, set_tab.setter(0)),
                button("Tab 2").on(event::click, set_tab.setter(1)),
                button("Tab 3").on(event::click, set_tab.setter(2)),
            ]
            .style("display: flex; gap: 10px; margin-bottom: 10px;"),
            
            Switch::new(tab, || div("Fallback (Should not happen)"))
                .case(0, || div("Content for Tab 1").style("padding: 10px; background: #eee;"))
                .case(1, || div("Content for Tab 2").style("padding: 10px; background: #ddd;"))
                .case(2, || div("Content for Tab 3").style("padding: 10px; background: #ccc;"))
        ]
    }

    #[component]
    pub fn IndexDemo() -> impl View {
        let (items, set_items) = signal(vec!["Item A", "Item B", "Item C"]);

        div![
            h3("Index For Loop Demo"),
            p("Optimized for list updates by index."),
            Index::new(items, |item, idx| {
                div![
                    strong(format!("{}: ", idx)),
                    // item is a ReadSignal<String> here
                    item
                ]
            }),
            button("Append Item").on(event::click, move |_| {
                set_items.update(|list| list.push("New Item"));
            })
            .style("margin-top: 10px;")
        ]
    }

    #[component]
    pub fn PortalDemo() -> impl View {
        let (show_modal, set_show_modal) = signal(false);

        div![
            h3("Portal Demo"),
            button("Toggle Modal").on(event::click, set_show_modal.updater(|v| *v = !*v)),
            
            Show::new(show_modal, move || {
                Portal::new(
                    div![
                        div![
                            h4("I am a Modal!"),
                            p("I am rendered via Portal directly into the body, but I share context!"),
                            button("Close").on(event::click, set_show_modal.setter(false))
                        ]
                        .style("background: white; padding: 20px; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.2); min-width: 300px;")
                    ]
                    .style("position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; background: rgba(0,0,0,0.5); display: flex; justify-content: center; align-items: center; z-index: 9999;")
                )
            })
        ]
    }

    #[component]
    pub fn FlowPage() -> impl View {
        div![
            h2("Control Flow"),
            ListDemo(),
            ShowDemo(),
            DynamicDemo(),
            SwitchDemo(),
            IndexDemo(),
            PortalDemo(),
        ]
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
                .on(event::click, || console_log("Clicked!")),
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
                    text(settings.notifications.map(|n| if n { "On" } else { "Off" })),
                ],
            ]
            .style("border: 1px solid #ccc; padding: 10px; margin-bottom: 10px;"),
            h4("Update Settings"),
            div![
                button("Toggle Theme").on(event::click, rx! {
                    settings.theme.update(|t| {
                        *t = if t == "Light" {
                            "Dark".to_string()
                        } else {
                            "Light".to_string()
                        }
                    })
                }),
                button("Toggle Notifications")
                    .on(event::click, settings.notifications.updater(|n| *n = !*n)),
                input()
                    .bind_value(settings.username)
                    .placeholder("Change username..."),
            ]
            .style("display: flex; gap: 10px;"),
        ]
    }

    #[component]
    pub fn QueryDemo() -> impl View {
        let val = use_query_signal("demo_val");

        div![
            h3("Query Signal Demo"),
            p(
                "This input is synced with the URL query parameter 'demo_val' using `use_query_signal`."
            ),
            div![
                input()
                    .bind_value(val) // Automatic two-way binding
                    .placeholder("Type here...")
                    .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px;"),
                button("Reset")
                    .on(event::click, val.setter(String::new()))
                    .style("padding: 8px 16px; cursor: pointer;")
            ]
            .style("display: flex; gap: 10px; margin: 10px 0; align-items: center;"),
            p![
                strong("Current Value: "),
                val // Signals implement Display
            ]
            .style("background: #f5f5f5; padding: 10px; border-radius: 4px;")
        ]
    }

    #[component]
    pub fn AuthGuard(children: Children) -> impl View {
        let settings = expect_context::<UserSettingsStore>();
        
        move || {
             if settings.username.get() != "Guest" {
                 children.clone()
             } else {
                 div![
                     h3("🔒 Restricted Access"),
                     p("This content is protected. Please go to 'Store Demo' and change your username to something other than 'Guest'."),
                 ].style("padding: 20px; background: #fff0f0; border: 1px solid #ffcccc; color: #cc0000;")
                 .into_any()
             }
        }
    }
}


mod styles {
    use super::*;

    #[component]
    pub fn SelectStyleDemo() -> impl View {
        div("Select a style above to see the comparison.")
    }

    #[component]
    pub fn BuilderDemo() -> impl View {
        // Nested structure using purely function calls
        div(
            div(
                // Use a tuple for multiple children if not using macros
                (
                    h3("Builder Style"),
                    p("This component is built using only function calls and method chaining."),
                    button("Click Me (Builder)")
                        .class("btn-builder")
                        .style("background-color: #e0f7fa; color: #006064; padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer;")
                        .on(event::click, |_| console_log("Builder button clicked!")),
                )
            )
            .style("padding: 20px; border: 1px solid #b2ebf2; border-radius: 8px; margin-bottom: 20px;")
        )
    }

    #[component]
    pub fn MacroDemo() -> impl View {
        let (count, set_count) = signal(0);

        // Note how clean the children list is: plain comma-separated values
        div![
            h3![ "Macro Style" ],
            p![ "This component uses macros for a more declarative, HTML-like feel." ],
            div![
                button![ "-" ]
                    .class("btn-macro")
                    .on(event::click, set_count.updater(|n| *n -= 1)),
                span![ " Count: ", count, " " ].style("margin: 0 10px; font-weight: bold;"),
                button![ "+" ]
                    .class("btn-macro")
                    .on(event::click, set_count.updater(|n| *n += 1)),
            ]
            .style("display: flex; align-items: center; margin-top: 10px;")
        ]
        .style("padding: 20px; border: 1px solid #ffccbc; background-color: #fffbe6; border-radius: 8px; margin-bottom: 20px;")
    }

    #[component]
    pub fn HybridDemo() -> impl View {
        let (is_active, set_active) = signal(false);

        div![
            h3("Hybrid Style (Recommended)"), // Function call for single child is fine too!
            p("Mix macros for structure and builder methods for attributes."),
            
            // "Toggle" Logic
            div![
                span(is_active.map(|v| if v { "State: Active" } else { "State: Inactive" }))
                    .style("margin-right: 15px;"),
                
                button(is_active.map(|v| if v { "Deactivate" } else { "Activate" }))
                    .on(event::click, move |_| set_active.update(|v| *v = !*v))
                    // Dynamic styling with builder pattern
                    .style(is_active.map(|v| {
                        if v {
                            "background-color: #ef5350; color: white;"
                        } else {
                            "background-color: #66bb6a; color: white;"
                        }
                    }))
                    .style("padding: 8px 16px; border: none; border-radius: 4px; cursor: pointer; transition: background-color 0.2s;")
            ]
        ]
        .style("padding: 20px; border: 1px solid #d1c4e9; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.05);")
    }
}

// --- Routing Definition ---


#[component]
fn SelectDemo() -> impl View {
    div("Select a demo above.")
}

#[derive(Route, Clone, PartialEq)]
enum AdvancedRoute {
    #[route("/", view = SelectDemo)]
    Index,
    #[route("/css", view = advanced::CssDemo)]
    Css,
    #[route("/store", view = advanced::StoreDemo)]
    Store,
    #[route("/query", view = advanced::QueryDemo, guard = advanced::AuthGuard)]
    Query,
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
enum StylesRoute {
    #[route("/", view = styles::SelectStyleDemo)]
    Index,
    #[route("/builder", view = styles::BuilderDemo)]
    Builder,
    #[route("/macro", view = styles::MacroDemo)]
    Macro,
    #[route("/hybrid", view = styles::HybridDemo)]
    Hybrid,
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/", view = HomePage)]
    Home,
    #[route("/basics", view = basics::BasicsPage)]
    Basics,
    #[route("/flow", view = flow_control::FlowPage)]
    Flow,
    #[route("/advanced/*", view = AdvancedLayout)]
    Advanced {
        #[nested]
        route: AdvancedRoute,
    },
    #[route("/styles/*", view = StylesLayout)]
    Styles {
        #[nested]
        route: StylesRoute,
    },
    #[route("/*", view = NotFoundPage)]
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
        Link(AppRoute::Home, "Home").class(&nav_link).active_class("active"),
        Link(AppRoute::Basics, "Basics").class(&nav_link).active_class("active"),
        Link(AppRoute::Flow, "Flow").class(&nav_link).active_class("active"),
        Link(AppRoute::Advanced {
            route: AdvancedRoute::Index,
        }, "Advanced")
        .class(&nav_link)
        .active_class("active"),
        Link(AppRoute::Styles {
            route: StylesRoute::Index,
        }, "Styles")
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
            Link("/advanced/css", "CSS Demo").class("tab"), // Support string literal
            Link(AppRoute::Advanced {
                route: AdvancedRoute::Store,
            }, "Store Demo")
            .class("tab"),
            Link(AppRoute::Advanced {
                route: AdvancedRoute::Query,
            }, "Query Param")
            .class("tab"),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 20px;"),
        // Delegate rendering to the route itself via RouteView
        route.render(),
    ]
}


#[component]
fn StylesLayout(route: StylesRoute) -> impl View {
    div![
        h2("Coding Style Comparison"),
        p("Silex supports multiple coding styles. Choose one below to see the difference."),
        div![
            Link("/styles/builder", "Builder Style").class("tab"),
            Link("/styles/macro", "Macro Style").class("tab"),
            Link("/styles/hybrid", "Hybrid Style").class("tab"),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 20px;"),
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
            li(Link(AppRoute::Basics, "Basics: Components, Props, Signals")),
            li(Link(AppRoute::Flow, "Flow Control: Loops, Conditions")),
            li(Link(AppRoute::Advanced {
                route: AdvancedRoute::Index,
            }, "Advanced: Router to Store & CSS")),
            li(Link(AppRoute::Styles {
                route: StylesRoute::Index,
            }, "Styles: Comparison of Builder vs Macro vs Hybrid")),
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
    mount_to_body(rx! {
        // Provide Global Store to the entire app tree
        provide_context(store);

        div![
            // Global Layout Shell
            NavBar(),
            // Root Router
            Router::new().match_route::<AppRoute>(),
        ]
    });
}
