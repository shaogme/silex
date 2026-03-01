use crate::advanced;
use crate::advanced::use_user_settings;
use crate::basics;
use crate::css;
use crate::flow_control;
use silex::prelude::*;
use silex::reexports::wasm_bindgen::JsCast;
use silex::reexports::web_sys::{HtmlElement, MouseEvent};

#[component]
fn SelectDemo() -> impl View {
    div("Select a demo above.")
}

#[derive(Route, Clone, PartialEq)]
pub enum AdvancedRoute {
    #[route("/", view = SelectDemo)]
    Index,
    #[route("/store", view = advanced::StoreDemo)]
    Store,
    #[route("/query", view = advanced::QueryDemo, guard = advanced::AuthGuard)]
    Query,
    #[route("/storage", view = advanced::StorageDemo)]
    Storage,
    #[route("/resource", view = advanced::ResourceDemo)]
    Resource,
    #[route("/mutation", view = advanced::MutationDemo)]
    Mutation,
    #[route("/suspense", view = advanced::SuspenseDemo)]
    Suspense,
    #[route("/generics", view = advanced::GenericsDemo)]
    Generics,
    #[route("/adaptive", view = advanced::AdaptiveReadDemo)]
    Adaptive,
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
pub enum CssRoute {
    #[route("/", view = css::StylingBasics)]
    Basics,
    #[route("/theming", view = css::Theming)]
    Theming,
    #[route("/advanced", view = css::AdvancedStyling)]
    Advanced,
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
pub enum AppRoute {
    #[route("/", view = HomePage)]
    Home,
    #[route("/basics", view = basics::BasicsPage)]
    Basics,
    #[route("/flow", view = flow_control::FlowPage)]
    Flow,
    #[route("/css/*", view = CssLayout)]
    Css {
        #[nested]
        route: CssRoute,
    },
    #[route("/advanced/*", view = AdvancedLayout)]
    Advanced {
        #[nested]
        route: AdvancedRoute,
    },
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

// --- Layout & App ---

styled! {
    pub StyledNav<nav> (
        children: Children,
        #[prop(default = "horizontal")] direction: &'static str
    ) {
        background: var(--slx-theme-surface);
        color: var(--slx-theme-text);
        border-bottom: 1px solid var(--slx-theme-border);
        padding: 12px 24px;
        margin-bottom: 20px;
        display: flex;
        gap: 15px;
        align-items: center;

        & a {
            color: var(--slx-theme-text);
            opacity: 0.8;
            padding: 8px 12px;
            border-radius: 4px;
            transition: background-color 0.2s;

            &:hover {
                background-color: var(--slx-theme-primary);
                color: white;
            }

            &.active {
                background-color: var(--slx-theme-primary);
                color: white;
                font-weight: bold;
            }
        }

        variants: {
            direction: {
                horizontal: { flex-direction: row; }
                vertical: { flex-direction: column; align-items: flex-start; }
            }
        }
    }
}

#[component]
pub fn NavBar() -> impl View {
    let settings = use_user_settings();

    StyledNav().direction("horizontal").children((
        Link(AppRoute::Home, "Home").active_class("active"),
        Link(AppRoute::Basics, "Basics").active_class("active"),
        Link(AppRoute::Flow, "Flow").active_class("active"),
        Link(
            AppRoute::Css {
                route: CssRoute::Basics,
            },
            "CSS",
        )
        .active_class("active"),
        Link(
            AppRoute::Advanced {
                route: AdvancedRoute::Index,
            },
            "Advanced",
        )
        .active_class("active"),
        button(settings.theme.map_fn(|t| if t == "Light" { "🌙" } else { "🌞" }))
            .on(
                event::click,
                move |_| {
                    settings.theme.update(|t| {
                        let new_theme = if t == "Light" { "Dark".to_string() } else { "Light".to_string() };
                        console_log(&format!("Button Click: switching to {}", new_theme));
                        *t = new_theme;
                    })
                }
            )
            .style("margin-left: auto; cursor: pointer; background: var(--slx-theme-border); border: none; padding: 8px 12px; border-radius: 50%; font-size: 1.2em; transition: all 0.3s; color: var(--slx-theme-text);")
            .on(event::mouseover, move |e: MouseEvent| {
                let target = e.target().unwrap().unchecked_into::<HtmlElement>();
                let _ = target.style().set_property("background", "rgba(255,255,255,0.2)");
            })
            .on(event::mouseout, move |e: MouseEvent| {
                let target = e.target().unwrap().unchecked_into::<HtmlElement>();
                let _ = target.style().set_property("background", "rgba(255,255,255,0.1)");
            }),
    ))
}

#[component]
fn AdvancedLayout(route: AdvancedRoute) -> impl View {
    div![
        h2("Advanced Features"),
        div![
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Store,
                },
                "Store Demo"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Query,
                },
                "Query Param"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Storage,
                },
                "Storage"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Resource,
                },
                "Resource"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Mutation,
                },
                "Mutation"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Suspense,
                },
                "Suspense"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Generics,
                },
                "Generics"
            )
            .class("tab"),
            Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Adaptive,
                },
                "Adaptive Read"
            )
            .class("tab"),
        ]
        .style("display: flex; gap: 10px; margin-bottom: 20px;"),
        route.render(),
    ]
}

#[component]
fn CssLayout(route: CssRoute) -> impl View {
    div![
        h2("CSS & Styling"),
        p(
            "Silex provides multiple ways to style your applications, from CSS-in-Rust to type-safe builders."
        ),
        div![
            Link("/css/", "Basics").class("tab"),
            Link("/css/theming", "Theme Engine").class("tab"),
            Link("/css/advanced", "Advanced CSS").class("tab"),
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
            li(Link(
                AppRoute::Css {
                    route: CssRoute::Basics,
                },
                "CSS: CSS-in-Rust, Themes, and Style Comparison"
            )),
            li(Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Index,
                },
                "Advanced: Store, Router, Resource, Mutation"
            )),
        ],
    ]
}
