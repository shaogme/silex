use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

// ==========================================
// 辅助组件
// ==========================================

/// 一个简单的卡片容器
#[component]
fn Card<'scope, Ctx>(#[context] context: Ctx, children: AnyView<'scope>) -> impl View<'scope> {
    Ok(div(children).style(
        sty()
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
    #[context] ctx: RouterContext<'scope>,
    to: T,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(Link(ctx, to)
        .children(children)
        .style(
            sty()
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
fn Home<'scope>(#[context] _ctx: RouterContext<'scope>) -> impl View<'scope> {
    div!(
        h2("🏠 Home Page"),
        p("Welcome to the Router Test Suite."),
        p("Use the navigation bar above to test different routing features."),
    )
}

#[component]
fn SearchPage<'scope>(#[context] ctx: RouterContext<'scope>) -> impl View<'scope> {
    let scope = ctx.scope();
    let search_term = Persistent::builder(scope, "q", error_handler)
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
                        sty()
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
                        sty()
                            .padding("8px 16px")?
                            .background("#f44336")?
                            .color(ColorName::White)?
                            .border("none")?
                            .border_radius(px(4))?
                            .cursor("pointer")?
                    ),
            )
            .style(sty().display("flex")?.gap(px(10))?.margin_bottom(px(20))?),
            div!(
                strong("Current Query Parameter (q): "),
                span(rx!(ctx; {
                    let value = $display_term.clone();
                    if value.is_empty() {
                        "None".to_string()
                    } else {
                        value
                    }
                }))
                .style(sty().color(hex("#e91e63"))?.font_family("monospace")?),
            ),
        ),
    )
    .build())
}

// --- 用户模块 (嵌套路由中的共享布局) ---

#[component]
fn CreateUser<'scope>(#[context] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Card(ctx, h3("🆕 Create New User Form")).build()
}

#[component]
fn UsersLayout<'scope>(
    #[context] ctx: RouterContext<'scope>,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        h2("👥 Users Module"),
        div!(
            NavLink(ctx, "/users").children("User List").build(),
            span("|").style(sty().margin("0 10px")?.color(hex("#ccc"))?),
            NavLink(ctx, "/users/new")
                .children("Create User (Static)")
                .build(),
        )
        .style(
            sty()
                .border_bottom("2px solid #eee")?
                .padding_bottom(px(10))?
                .margin_bottom(px(20))?
        ),
        children,
    ))
}

fn user_detail_path(id: u32) -> SilexResult<RoutePath> {
    let encoded = <u32 as PathParam>::encode_segment(&id)
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    RoutePath::new(format!("/users/{encoded}"))
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))
}

#[component]
fn UserList<'scope>(#[context] ctx: RouterContext<'scope>) -> impl View<'scope> {
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
                .style(sty().text_decoration("none")?.color(hex("#2196f3"))?)
                .active_class("active-user")
                .build())
            .style(sty().margin("5px 0")?))
        })
        .collect::<SilexResult<Vec<_>>>()?;

    Ok(div!(
        h3("Select a User:"),
        ul(user_links).style(sty().list_style("none")?.padding("0")?),
    ))
}

#[component]
fn UserDetail<'scope>(#[context] ctx: RouterContext<'scope>, id: u32) -> impl View<'scope> {
    let navigator = ctx.navigator;
    let path = ctx.path;

    Ok(Card(
        ctx,
        div!(
            div!(
                h3(format!("User Profile: #{}", id)),
                button("Go Back")
                    .on_click(move |_| {
                        navigator.push("/users")?;
                        Ok(())
                    })
                    .style(
                        sty()
                            .font_size(rem(0.8))?
                            .padding("5px 10px")?
                            .cursor("pointer")?
                    ),
            )
            .style(
                sty()
                    .display("flex")?
                    .justify_content("space-between")?
                    .align_items("center")?
            ),
            hr().style(
                sty()
                    .border("0")?
                    .border_top("1px solid #eee")?
                    .margin("15px 0")?
            ),
            p!(
                strong("Current Path: "),
                span(path).style(sty().font_family("monospace")?),
            ),
            div!(p(format!(
                "This component is rendered with strict prop id: {}",
                id
            )))
            .style(
                sty()
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
fn NotFound<'scope>(#[context] ctx: RouterContext<'scope>) -> impl View<'scope> {
    Ok(div!(
        h1("404"),
        p("Page not found."),
        Link(ctx, "/")
            .children("Return Home")
            .style(sty().color(hex("#2196f3"))?.text_decoration("underline")?)
            .build(),
    )
    .style(
        sty()
            .text_align(TextAlignKeyword::Center)?
            .padding("50px")?
            .color(hex("#d32f2f"))?,
    ))
}

// --- 主布局 ---

#[component]
fn MainLayout<'scope>(
    #[context] ctx: RouterContext<'scope>,
    home_path: RoutePath,
    users_path: RoutePath,
    search_path: RoutePath,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Ok(div!(
        header!(
            h1("🚀 Silex Router").style(
                sty()
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
            sty()
                .display("flex")?
                .align_items("center")?
                .justify_content("space-between")?
                .padding("20px 0")?
                .border_bottom("1px solid #eee")?
        ),
        ::silex::html::main(children).style(sty().padding("20px 0")?),
        footer!(p("Built with Silex & Rust")).style(
            sty()
                .margin_top(px(50))?
                .padding_top(px(20))?
                .border_top("1px solid #eee")?
                .text_align(TextAlignKeyword::Center)?
                .color(hex("#999"))?
                .font_size(rem(0.8))?
        ),
    )
    .style(
        sty()
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
fn App<'scope>(#[context] context: SilexContext<'scope>) -> impl View<'scope> {
    let users = routes!(UsersRoutes {
        list "/" => move |ctx| UserList(ctx).build(),
        create "/new" => move |ctx| CreateUser(ctx).build(),
        detail "/:id" => move |ctx, id: u32| UserDetail(ctx, id).build(),
    })
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?
    .at("/users")
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let routes = routes!(AppRoutes {
        home "/" => move |ctx| Home(ctx).build(),
        search "/search" => move |ctx| SearchPage(ctx).build(),
        not_found "/*" => move |ctx| NotFound(ctx).build(),
    })
    .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    let home_path = routes
        .home()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let users_path = users
        .list()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let search_path = routes
        .search()
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;
    let table = routes
        .table()
        .nest(users.prefix(), users.table(), move |ctx, outlet| {
            UsersLayout(ctx).children(outlet).build()
        })
        .map_err(|error| SilexError::recoverable(SilexErrorKind::Framework(error.to_string())))?;

    Ok(Router(context)
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

fn mount_router_view<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let scope = context.scope();
    let error_handler = scope.error_handler(|error: SilexError| {
        web_sys::console::error_1(&error.to_string().into());
    })?;
    let silex_context = SilexContext::new(scope, error_handler);
    context.mount(App(silex_context).build(), error_handler)
}
