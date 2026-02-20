use crate::advanced;
use crate::basics;
use crate::flow_control;
use silex::prelude::*;

#[component]
fn SelectDemo() -> impl View {
    div("Select a demo above.")
}

#[derive(Route, Clone, PartialEq)]
pub enum AdvancedRoute {
    #[route("/", view = SelectDemo)]
    Index,
    #[route("/css", view = advanced::CssDemo)]
    Css,
    #[route("/store", view = advanced::StoreDemo)]
    Store,
    #[route("/query", view = advanced::QueryDemo, guard = advanced::AuthGuard)]
    Query,
    #[route("/resource", view = advanced::ResourceDemo)]
    Resource,
    #[route("/mutation", view = advanced::MutationDemo)]
    Mutation,
    #[route("/suspense", view = advanced::SuspenseDemo)]
    Suspense,
    #[route("/*", view = NotFoundPage)]
    NotFound,
}

#[derive(Route, Clone, PartialEq)]
pub enum StylesRoute {
    #[route("/", view = basics::SelectStyleDemo)]
    Index,
    #[route("/builder", view = basics::BuilderDemo)]
    Builder,
    #[route("/macro", view = basics::MacroDemo)]
    Macro,
    #[route("/hybrid", view = basics::HybridDemo)]
    Hybrid,
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
pub fn NavBar() -> impl View {
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
        Link(AppRoute::Home, "Home").class(nav_link).active_class("active"),
        Link(AppRoute::Basics, "Basics").class(nav_link).active_class("active"),
        Link(AppRoute::Flow, "Flow").class(nav_link).active_class("active"),
        Link(AppRoute::Advanced {
            route: AdvancedRoute::Index,
        }, "Advanced")
        .class(nav_link)
        .active_class("active"),
        Link(AppRoute::Styles {
            route: StylesRoute::Index,
        }, "Styles")
        .class(nav_link)
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
            li(Link(
                AppRoute::Advanced {
                    route: AdvancedRoute::Index,
                },
                "Advanced: Router to Store & CSS"
            )),
            li(Link(
                AppRoute::Styles {
                    route: StylesRoute::Index,
                },
                "Styles: Comparison of Builder vs Macro vs Hybrid"
            )),
        ],
    ]
}
