use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

router! {
    pub enum UsersRoute {
        List => "/",
        Create => "/new",
        Detail { id: u32 } => "/:id",
    }
}

router! {
    pub enum AppRoute {
        Home => "/",
        Search => "/search",
        Users(UsersRoute) {
            prefix: "/users";
            layout: |ctx, outlet| UsersLayout(ctx).children(outlet).build();
        },
        NotFound => "/*",
    }
}

// ==========================================
// 辅助组件
// ==========================================

/// 一个简单的卡片容器
#[component]
fn Card<'scope, Ctx>(#[ctx] ctx: Ctx, children: AnyView<'scope>) -> impl View<'scope> {
    Ok(div(children).style(
        sty(ctx)
            .border("1px solid #ddd")?
            .border_radius(px(8))?
            .padding("20px")?
            .margin("10px 0")?
            .background("white")?
            .box_shadow("0 2px 4px rgba(0,0,0,0.05)")?,
    ))
}

/// 导航链接样式封装
#[component]
fn NavLink<'scope, T: ToRoute + Clone + 'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    to: T,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(Link(ctx, to)
        .children(children)
        .style(
            sty(ctx)
                .margin_right(px(15))?
                .text_decoration("none")?
                .color(hex("#666"))?
                .padding("5px 10px")?
                .border_radius(px(4))?
                .transition("all 0.2s")?,
        )
        .active_class("nav-active")
        .build())
}

// ==========================================

#[component]
fn Home<'scope>(#[ctx] _ctx: RouterContext<'scope>) -> impl View<'scope> {
    div!(
        h2("🏠 Home Page"),
        p("Welcome to the Router Test Suite."),
        p("Use the navigation bar above to test different routing features."),
    )
}

#[component]
fn SearchPage<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    let owner = ctx.owner();
    let search_term = Persistent::builder(owner, "q", error_handler)
        .query(ctx)
        .string()
        .default(String::new())
        .build()?;
    let display_term = search_term;

    Ok(Card(
        ctx,
        div!(
            h2("🔍 Search Query Test"),
            p("Type in the input below. The URL query parameter 'q' will update automatically!"),
            div!(
                input()
                    .attr("type", "text")
                    .placeholder("Type search term...")
                    .bind_value(search_term)
                    .style(
                        sty(ctx)
                            .padding("8px")?
                            .border("1px solid #ccc")?
                            .border_radius(px(4))?
                            .flex("1")?
                    ),
                button("Clear")
                    .on_click(move |_| {
                        search_term.set(String::new())?;
                        Ok(())
                    })
                    .style(
                        sty(ctx)
                            .padding("8px 16px")?
                            .background("#f44336")?
                            .color(ColorName::White)?
                            .border("none")?
                            .border_radius(px(4))?
                            .cursor("pointer")?
                    ),
            )
            .style(
                sty(ctx)
                    .display("flex")?
                    .gap(px(10))?
                    .margin_bottom(px(20))?
            ),
            div!(
                strong("Current Query Parameter (q): "),
                span(rx!(ctx; {
                    let value = $display_term.clone();
                    if value.is_empty() {
                        "None".to_string()
                    } else {
                        value
                    }
                })?)
                .style(sty(ctx).color(hex("#e91e63"))?.font_family("monospace")?),
            ),
        ),
    )
    .build())
}

// --- 用户模块 (嵌套路由中的共享布局) ---

#[component]
fn CreateUser<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Card(ctx, h3("🆕 Create New User Form")).build()
}

#[component]
fn UsersLayout<'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        h2("👥 Users Module"),
        div!(
            NavLink(ctx, "/users").children("User List").build(),
            span("|").style(sty(ctx).margin("0 10px")?.color(hex("#ccc"))?),
            NavLink(ctx, "/users/new")
                .children("Create User (Static)")
                .build(),
        )
        .style(
            sty(ctx)
                .border_bottom("2px solid #eee")?
                .padding_bottom(px(10))?
                .margin_bottom(px(20))?
        ),
        children,
    ))
}

fn user_detail_path(id: u32) -> SilexResult<RoutePath> {
    AppRoute::Users(UsersRoute::Detail { id })
        .path()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))
}

#[component]
fn UserList<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    let users = vec![
        (1, "Alice"),
        (2, "Bob"),
        (3, "Charlie"),
        (42, "Silex Expert"),
    ];

    let user_links = users
        .into_iter()
        .map(|(id, name)| {
            let path = user_detail_path(id)?;
            Ok(li(Link(ctx, path)
                .children(format!("👤 {} (ID: {})", name, id))
                .style(sty(ctx).text_decoration("none")?.color(hex("#2196f3"))?)
                .active_class("active-user")
                .build())
            .style(sty(ctx).margin("5px 0")?))
        })
        .collect::<SilexResult<Vec<_>>>()?;

    Ok(div!(
        h3("Select a User:"),
        ul(user_links).style(sty(ctx).list_style("none")?.padding("0")?),
    ))
}

#[component]
fn UserDetail<'scope>(#[ctx] ctx: RouterContext<'scope>, id: u32) -> impl View<'scope> {
    let navigator = ctx.navigator;
    let path = ctx.path;
    let owner = ctx.owner();
    let error_handler = ctx.error_reporter();
    let (page_signal, _set_page_signal) = owner.signal(id)?;
    let page_computed = owner.computed(
        move || page_signal.get().map(|value| value.saturating_mul(2)),
        error_handler,
    )?;
    let _page_effect = owner.effect(
        move || {
            let _ = page_computed.get()?;
            Ok(())
        },
        error_handler,
    )?;
    owner.on_cleanup(|| Ok(()), error_handler)?;

    Ok(Card(
        ctx,
        div!(
            div!(
                h3(format!("User Profile: #{}", id)),
                button("Go Back")
                    .on_click(move |_| {
                        navigator.push(AppRoute::Users(UsersRoute::List).path().map_err(
                            |error| {
                                SilexError::recoverable(SilexErrorKind::Framework(
                                    error.to_string(),
                                ))
                            },
                        )?)?;
                        Ok(())
                    })
                    .style(
                        sty(ctx)
                            .font_size(rem(0.8))?
                            .padding("5px 10px")?
                            .cursor("pointer")?
                    ),
            )
            .style(
                sty(ctx)
                    .display("flex")?
                    .justify_content("space-between")?
                    .align_items("center")?
            ),
            hr().style(
                sty(ctx)
                    .border("0")?
                    .border_top("1px solid #eee")?
                    .margin("15px 0")?
            ),
            p!(
                strong("Current Path: "),
                span(path).style(sty(ctx).font_family("monospace")?),
            ),
            div!(p(format!(
                "This component is rendered with strict prop id: {}",
                id
            )))
            .style(
                sty(ctx)
                    .background("#f5f5f5")?
                    .padding("10px")?
                    .border_radius(px(4))?
                    .margin_top(px(10))?
            ),
        ),
    )
    .build())
}

#[component]
fn NotFound<'scope>(#[ctx] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("404"),
        p("Page not found."),
        Link(ctx, "/")
            .children("Return Home")
            .style(
                sty(ctx)
                    .color(hex("#2196f3"))?
                    .text_decoration("underline")?
            )
            .build(),
    )
    .style(
        sty(ctx)
            .text_align(TextAlignKeyword::Center)?
            .padding("50px")?
            .color(hex("#d32f2f"))?,
    ))
}

// --- 主布局 ---

#[component]
fn MainLayout<'scope>(
    #[ctx] ctx: RouterContext<'scope>,
    home_path: RoutePath,
    users_path: RoutePath,
    search_path: RoutePath,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        header!(
            h1("🚀 Silex Router").style(
                sty(ctx)
                    .margin("0")?
                    .font_size(rem(1.5))?
                    .color(hex("#2c3e50"))?
            ),
            nav!(
                NavLink(ctx, home_path).children("Home").build(),
                NavLink(ctx, users_path).children("Users").build(),
                NavLink(ctx, search_path).children("Search").build(),
                NavLink(ctx, "/nowhere").children("404 Test").build(),
            )
        )
        .style(
            sty(ctx)
                .display("flex")?
                .align_items("center")?
                .justify_content("space-between")?
                .padding("20px 0")?
                .border_bottom("1px solid #eee")?
        ),
        ::silex::html::main(children).style(sty(ctx).padding("20px 0")?),
        footer!(p("Built with Silex & Rust")).style(
            sty(ctx)
                .margin_top(px(50))?
                .padding_top(px(20))?
                .border_top("1px solid #eee")?
                .text_align(TextAlignKeyword::Center)?
                .color(hex("#999"))?
                .font_size(rem(0.8))?
        ),
    )
    .style(
        sty(ctx)
            .font_family("sans-serif")?
            .max_width(px(800))?
            .margin("0 auto")?
            .color(hex("#333"))?,
    ))
}

// ==========================================
// App 入口
// ==========================================

#[component]
fn App<'scope>(#[ctx] ctx: SilexContext<'scope>) -> impl View<'scope> {
    let home_path = AppRoute::Home
        .path()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let users_path = AppRoute::Users(UsersRoute::List)
        .path()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let search_path = AppRoute::Search
        .path()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let table = AppRoute::table(move |route, ctx| match route {
        AppRoute::Home => Home(ctx).build().into_any(),
        AppRoute::Search => SearchPage(ctx).build().into_any(),
        AppRoute::Users(UsersRoute::List) => UserList(ctx).build().into_any(),
        AppRoute::Users(UsersRoute::Create) => CreateUser(ctx).build().into_any(),
        AppRoute::Users(UsersRoute::Detail { id }) => UserDetail(ctx, id).build().into_any(),
        AppRoute::NotFound => NotFound(ctx).build().into_any(),
    })
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    Ok(Router(ctx)
        .routes(table)
        .layout(move |ctx, outlet| {
            MainLayout(
                ctx,
                home_path.clone(),
                users_path.clone(),
                search_path.clone(),
            )
            .children(outlet)
            .build()
        })
        .build())
}

/// Mount the Router demo into the conventional `#app` target.
pub fn mount_router() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app", CleanupSink::console())?;
    inject_router_styles();
    bootstrap.mount(Runtime::new(), mount_router_view)?;
    bootstrap.into_js_host()
}

/// Mount the Router demo into a caller-provided target node.
pub fn mount_router_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target, CleanupSink::console());
    inject_router_styles();
    bootstrap.mount(Runtime::new(), mount_router_view)?;
    bootstrap.into_js_host()
}

fn inject_router_styles() {
    silex::macros::inject_css! {
        .nav-active {
            background-color: #e3f2fd !important;
            color: #1976d2 !important;
            font-weight: bold;
        }
    };
}

fn mount_router_view<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let owner = ctx.access();
    let error_handler = owner.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    let silex_ctx = SilexContext::new(owner, error_handler.view());
    ctx.mount(App(silex_ctx).build(), error_handler)
}
