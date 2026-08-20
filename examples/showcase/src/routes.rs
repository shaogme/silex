use crate::{AdvancedRoute, AppRoute, CssRoute};
use crate::{advanced::UserSettingsStore, css::AppTheme};
use silex::prelude::*;

fn route_path(route: AppRoute) -> SilexResult<RoutePath> {
    route
        .path()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))
}

#[component]
pub fn SelectDemo<'scope>(#[ctx] _ctx: RouterContext<'scope>) -> impl View<'scope> {
    div("Select a demo above.")
}

#[component]
pub fn NavBar<'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
) -> impl View<'scope> {
    Ok(nav!(
        Link(ctx, route_path(AppRoute::Home)?)
            .children("Home")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::Basics)?)
            .children("Basics")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::Flow)?)
            .children("Flow")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::I18n)?)
            .children("I18n")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::Net)?)
            .children("Net")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::Persistence)?)
            .children("Persistence")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::Css(CssRoute::Basics))?)
            .children("CSS")
            .active_class("active")
            .build(),
        Link(ctx, route_path(AppRoute::Advanced(AdvancedRoute::Index))?)
            .children("Advanced")
            .active_class("active")
            .build(),
        button(rx!(ctx; if $(settings.theme) == "Light" { "Dark" } else { "Light" })?)
            .on_click(move |_| {
                settings.theme.update(|theme| {
                    *theme = if theme == "Light" {
                        "Dark".to_string()
                    } else {
                        "Light".to_string()
                    };
                })?;
                Ok(())
            })
            .style(
                sty(ctx)
                    .margin_left(AUTO)?
                    .cursor(CursorKeyword::Pointer)?
                    .background(AppTheme::BORDER)?
                    .border(NONE)?
                    .padding(padding::block_inline(px(8), px(12)))?
                    .border_radius(px(6))?
                    .color(AppTheme::TEXT)?,
            ),
    )
    .style(
        sty(ctx)
            .display("flex")?
            .flex_wrap(FlexWrapKeyword::Wrap)?
            .align_items("center")?
            .gap(px(8))?
            .padding("12px 24px")?
            .margin_bottom(px(20))?
            .background(AppTheme::SURFACE)?
            .color(AppTheme::TEXT)?
            .border_bottom(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?,
    ))
}

#[component]
pub fn AppLayout<'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    outlet: AnyView<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
) -> impl View<'scope> {
    Ok(div!(
        NavBar(ctx, settings).build(),
        main(outlet).style(
            sty(ctx)
                .max_width(px(1200))?
                .margin("0 auto")?
                .padding("0 20px 40px")?
        ),
    ))
}

#[component]
pub fn AdvancedLayout<'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    outlet: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        h2("Advanced Features"),
        div!(
            Link(ctx, route_path(AppRoute::Advanced(AdvancedRoute::Store))?)
                .children("Store Demo")
                .class("tab")
                .build(),
            Link(ctx, route_path(AppRoute::Advanced(AdvancedRoute::Query))?)
                .children("Query Param")
                .class("tab")
                .build(),
            Link(ctx, route_path(AppRoute::Advanced(AdvancedRoute::Storage))?)
                .children("Storage")
                .class("tab")
                .build(),
            Link(
                ctx,
                route_path(AppRoute::Advanced(AdvancedRoute::Resource))?
            )
            .children("Resource")
            .class("tab")
            .build(),
            Link(
                ctx,
                route_path(AppRoute::Advanced(AdvancedRoute::Mutation))?
            )
            .children("Mutation")
            .class("tab")
            .build(),
            Link(
                ctx,
                route_path(AppRoute::Advanced(AdvancedRoute::Suspense))?
            )
            .children("Suspense")
            .class("tab")
            .build(),
            Link(
                ctx,
                route_path(AppRoute::Advanced(AdvancedRoute::Generics))?
            )
            .children("Generics")
            .class("tab")
            .build(),
            Link(
                ctx,
                route_path(AppRoute::Advanced(AdvancedRoute::Adaptive))?
            )
            .children("Adaptive Read")
            .class("tab")
            .build(),
        )
        .style(
            sty(ctx)
                .display("flex")?
                .flex_wrap(FlexWrapKeyword::Wrap)?
                .gap(px(10))?
                .margin_bottom(px(20))?
        ),
        outlet,
    ))
}

#[component]
pub fn CssLayout<'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    outlet: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        h2("CSS & Styling"),
        p(
            "Silex provides multiple ways to style your applications, from CSS-in-Rust to type-safe builders."
        ),
        div!(
            Link(ctx, route_path(AppRoute::Css(CssRoute::Basics))?)
                .children("Basics")
                .class("tab")
                .build(),
            Link(ctx, route_path(AppRoute::Css(CssRoute::Theming))?)
                .children("Theme Engine")
                .class("tab")
                .build(),
            Link(ctx, route_path(AppRoute::Css(CssRoute::Advanced))?)
                .children("Advanced CSS")
                .class("tab")
                .build(),
        )
        .style(
            sty(ctx)
                .display("flex")?
                .flex_wrap(FlexWrapKeyword::Wrap)?
                .gap(px(10))?
                .margin_bottom(px(20))?
        ),
        outlet,
    ))
}

#[component]
pub fn NotFoundPage<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("404 - Page Not Found"),
        Link(ctx, route_path(AppRoute::Home)?)
            .children("Return Home")
            .class("tab")
            .build(),
    )
    .style(sty(ctx).color(ColorName::Red)?.padding("20px")?))
}

#[component]
pub fn HomePage<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("Welcome to Silex Showcase"),
        p("This example application demonstrates the core features of the Silex framework."),
        ul!(
            li(Link(ctx, route_path(AppRoute::Basics)?)
                .children("Basics: Components, Props, Signals")
                .build()),
            li(Link(ctx, route_path(AppRoute::Flow)?)
                .children("Flow Control: Loops, Conditions")
                .build()),
            li(Link(ctx, route_path(AppRoute::I18n)?)
                .children("I18n: Locale, fallback, and plural messages")
                .build()),
            li(Link(ctx, route_path(AppRoute::Css(CssRoute::Basics))?)
                .children("CSS: CSS-in-Rust, Themes, and Style Comparison")
                .build()),
            li(Link(ctx, route_path(AppRoute::Net)?)
                .children("Net: HttpClient, WebSocket, EventStream")
                .build()),
            li(Link(ctx, route_path(AppRoute::Persistence)?)
                .children("Persistence: WebStorage, Query, Sync, Codecs")
                .build()),
            li(
                Link(ctx, route_path(AppRoute::Advanced(AdvancedRoute::Index))?)
                    .children("Advanced: Store, Router, Resource, Mutation")
                    .build()
            ),
        ),
    ))
}
