use crate::{advanced::UserSettingsStore, css::AppTheme};
use silex::prelude::*;

#[component]
pub fn SelectDemo<'scope>() -> impl View<'scope> {
    div("Select a demo above.")
}

#[component]
pub fn NavBar<'scope>(
    ctx: RouterContext<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(nav!(
        Link(ctx, "/").children("Home").active_class("active").build(),
        Link(ctx, "/basics")
            .children("Basics")
            .active_class("active")
            .build(),
        Link(ctx, "/flow")
            .children("Flow")
            .active_class("active")
            .build(),
        Link(ctx, "/i18n")
            .children("I18n")
            .active_class("active")
            .build(),
        Link(ctx, "/net")
            .children("Net")
            .active_class("active")
            .build(),
        Link(ctx, "/persistence")
            .children("Persistence")
            .active_class("active")
            .build(),
        Link(ctx, "/css/")
            .children("CSS")
            .active_class("active")
            .build(),
        Link(ctx, "/advanced/")
            .children("Advanced")
            .active_class("active")
            .build(),
        button(rx!(ctx.scope(); error_handler; if $(settings.theme) == "Light" { "Dark" } else { "Light" }))
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
                sty()
                    .margin_left(AUTO)?
                    .cursor(CursorKeyword::Pointer)?
                    .background(AppTheme::BORDER)?
                    .border(NONE)?
                    .padding(padding::block_inline(px(8), px(12)))?
                    .border_radius(px(6))?
                    .color(AppTheme::TEXT)?,
            ),
    )
    .style(sty().display("flex")?.flex_wrap(FlexWrapKeyword::Wrap)?.align_items("center")?.gap(px(8))?.padding("12px 24px")?.margin_bottom(px(20))?.background(AppTheme::SURFACE)?.color(AppTheme::TEXT)?.border_bottom(border(px(1), BorderStyleKeyword::Solid, AppTheme::BORDER))?))
}

#[component]
pub fn AppLayout<'scope>(
    ctx: RouterContext<'scope>,
    outlet: AnyView<'scope>,
    settings: UserSettingsStore<'scope, 'scope>,
    error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        NavBar(ctx, settings, error_handler).build(),
        main(outlet).style(
            sty()
                .max_width(px(1200))?
                .margin("0 auto")?
                .padding("0 20px 40px")?
        ),
    ))
}

#[component]
pub fn AdvancedLayout<'scope>(
    ctx: RouterContext<'scope>,
    outlet: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        h2("Advanced Features"),
        div!(
            Link(ctx, "/advanced/store")
                .children("Store Demo")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/query")
                .children("Query Param")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/storage")
                .children("Storage")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/resource")
                .children("Resource")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/mutation")
                .children("Mutation")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/suspense")
                .children("Suspense")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/generics")
                .children("Generics")
                .class("tab")
                .build(),
            Link(ctx, "/advanced/adaptive")
                .children("Adaptive Read")
                .class("tab")
                .build(),
        )
        .style(
            sty()
                .display("flex")?
                .flex_wrap(FlexWrapKeyword::Wrap)?
                .gap(px(10))?
                .margin_bottom(px(20))?
        ),
        outlet,
    ))
}

#[component]
pub fn CssLayout<'scope>(ctx: RouterContext<'scope>, outlet: AnyView<'scope>) -> impl View<'scope> {
    Ok(div!(
        h2("CSS & Styling"),
        p(
            "Silex provides multiple ways to style your applications, from CSS-in-Rust to type-safe builders."
        ),
        div!(
            Link(ctx, "/css/").children("Basics").class("tab").build(),
            Link(ctx, "/css/theming")
                .children("Theme Engine")
                .class("tab")
                .build(),
            Link(ctx, "/css/advanced")
                .children("Advanced CSS")
                .class("tab")
                .build(),
        )
        .style(
            sty()
                .display("flex")?
                .flex_wrap(FlexWrapKeyword::Wrap)?
                .gap(px(10))?
                .margin_bottom(px(20))?
        ),
        outlet,
    ))
}

#[component]
pub fn NotFoundPage<'scope>(ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("404 - Page Not Found"),
        Link(ctx, "/").children("Return Home").class("tab").build(),
    )
    .style(sty().color(ColorName::Red)?.padding("20px")?))
}

#[component]
pub fn HomePage<'scope>(ctx: RouterContext<'scope>) -> impl View<'scope> {
    div!(
        h1("Welcome to Silex Showcase"),
        p("This example application demonstrates the core features of the Silex framework."),
        ul!(
            li(Link(ctx, "/basics")
                .children("Basics: Components, Props, Signals")
                .build()),
            li(Link(ctx, "/flow")
                .children("Flow Control: Loops, Conditions")
                .build()),
            li(Link(ctx, "/i18n")
                .children("I18n: Locale, fallback, and plural messages")
                .build()),
            li(Link(ctx, "/css/")
                .children("CSS: CSS-in-Rust, Themes, and Style Comparison")
                .build()),
            li(Link(ctx, "/net")
                .children("Net: HttpClient, WebSocket, EventStream")
                .build()),
            li(Link(ctx, "/persistence")
                .children("Persistence: WebStorage, Query, Sync, Codecs")
                .build()),
            li(Link(ctx, "/advanced/")
                .children("Advanced: Store, Router, Resource, Mutation")
                .build()),
        ),
    )
}
