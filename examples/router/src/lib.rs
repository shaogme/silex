use silex::bootstrap::{BootstrapError, BrowserBootstrap, JsAppHost};
use silex::prelude::*;
use silex::reexports::*;

// ==========================================
// 辅助组件
// ==========================================

/// 一个简单的卡片容器
#[component]
fn Card<'scope>(children: AnyView<'scope>) -> impl View<'scope> {
    div(children)
        .style("border: 1px solid #ddd; border-radius: 8px; padding: 20px; margin: 10px 0; background: white; box-shadow: 0 2px 4px rgba(0,0,0,0.05);")
}

/// 导航链接样式封装
#[component]
fn NavLink<'scope, T: ToRoute + Clone + 'scope>(
    ctx: RouterContext<'scope>,
    to: T,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    Link(ctx, to)
        .children(children)
        .style("margin-right: 15px; text-decoration: none; color: #666; padding: 5px 10px; border-radius: 4px; transition: all 0.2s;")
        .active_class("nav-active")
        .build()
}

// ==========================================

#[component]
fn Home<'scope>() -> impl View<'scope> {
    div!(
        h2("🏠 Home Page"),
        p("Welcome to the Router Test Suite."),
        p("Use the navigation bar above to test different routing features."),
    )
}

#[component]
fn SearchPage<'scope>(
    ctx: RouterContext<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let scope = ctx.scope();
    let search_term = Persistent::builder(scope, "q", error_handler)
        .query(&ctx)
        .string()
        .default(String::new())
        .build();
    let display_term = search_term;

    Card(div!(
        h2("🔍 Search Query Test"),
        p("Type in the input below. The URL query parameter 'q' will update automatically!"),
        div!(
            input()
                .attr("type", "text")
                .placeholder("Type search term...")
                .bind_value(search_term)
                .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; flex: 1;"),
            button("Clear")
                .on_click(move |_| {
                    search_term.set(String::new());
                    Ok(())
                })
                .style("padding: 8px 16px; background: #f44336; color: white; border: none; border-radius: 4px; cursor: pointer;"),
        )
        .style("display: flex; gap: 10px; margin-bottom: 20px;"),
        div!(
            strong("Current Query Parameter (q): "),
            span(rx!(scope; {
                let value = $display_term.clone();
                if value.is_empty() {
                    "None".to_string()
                } else {
                    value
                }
            }))
            .style("color: #e91e63; font-family: monospace;"),
        ),
    ))
    .build()
}

// --- 用户模块 (嵌套路由中的共享布局) ---

#[component]
fn CreateUser<'scope>() -> impl View<'scope> {
    Card(h3("🆕 Create New User Form")).build()
}

#[component]
fn UsersLayout<'scope>(
    ctx: RouterContext<'scope>,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    div!(
        h2("👥 Users Module"),
        div!(
            NavLink(ctx, "/users").children("User List").build(),
            span("|").style("margin: 0 10px; color: #ccc;"),
            NavLink(ctx, "/users/new")
                .children("Create User (Static)")
                .build(),
        )
        .style("border-bottom: 2px solid #eee; padding-bottom: 10px; margin-bottom: 20px;"),
        children,
    )
}

fn user_detail_path(id: u32) -> RoutePath {
    let encoded = <u32 as PathParam>::encode_segment(&id)
        .unwrap_or_else(|error| panic!("failed to encode user id: {error}"));
    RoutePath::new(format!("/users/{encoded}"))
        .unwrap_or_else(|error| panic!("failed to build user route: {error}"))
}

#[component]
fn UserList<'scope>(ctx: RouterContext<'scope>) -> impl View<'scope> {
    let users = vec![
        (1, "Alice"),
        (2, "Bob"),
        (3, "Charlie"),
        (42, "Silex Expert"),
    ];

    div!(
        h3("Select a User:"),
        ul(users
            .into_iter()
            .map(|(id, name)| {
                li(Link(ctx, user_detail_path(id))
                    .children(format!("👤 {} (ID: {})", name, id))
                    .style("text-decoration: none; color: #2196f3;")
                    .active_class("active-user")
                    .build())
                .style("margin: 5px 0;")
            })
            .collect::<Vec<_>>())
        .style("list-style: none; padding: 0;"),
    )
}

#[component]
fn UserDetail<'scope>(ctx: RouterContext<'scope>, id: u32) -> impl View<'scope> {
    let navigator = ctx.navigator;
    let path = ctx.path;

    Card(div!(
        div!(
            h3(format!("User Profile: #{}", id)),
            button("Go Back")
                .on_click(move |_| {
                    navigator.push("/users");
                    Ok(())
                })
                .style("font-size: 0.8rem; padding: 5px 10px; cursor: pointer;"),
        )
        .style("display: flex; justify-content: space-between; align-items: center;"),
        hr().style("border: 0; border-top: 1px solid #eee; margin: 15px 0;"),
        p!(
            strong("Current Path: "),
            span(path).style("font-family: monospace;"),
        ),
        div!(p(format!(
            "This component is rendered with strict prop id: {}",
            id
        )))
        .style("background: #f5f5f5; padding: 10px; border-radius: 4px; margin-top: 10px;"),
    ))
    .build()
}

#[component]
fn NotFound<'scope>(ctx: RouterContext<'scope>) -> impl View<'scope> {
    div!(
        h1("404"),
        p("Page not found."),
        Link(ctx, "/")
            .children("Return Home")
            .style("color: #2196f3; text-decoration: underline;")
            .build(),
    )
    .style("text-align: center; padding: 50px; color: #d32f2f;")
}

// --- 主布局 ---

#[component]
fn MainLayout<'scope>(
    ctx: RouterContext<'scope>,
    home_path: RoutePath,
    users_path: RoutePath,
    search_path: RoutePath,
    #[chain] children: AnyView<'scope>,
) -> impl View<'scope> {
    div!(
        header!(
            h1("🚀 Silex Router").style("margin: 0; font-size: 1.5rem; color: #2c3e50;"),
            nav!(
                NavLink(ctx, home_path).children("Home").build(),
                NavLink(ctx, users_path).children("Users").build(),
                NavLink(ctx, search_path).children("Search").build(),
                NavLink(ctx, "/nowhere").children("404 Test").build(),
            )
        )
        .style("display: flex; align-items: center; justify-content: space-between; padding: 20px 0; border-bottom: 1px solid #eee;"),
        ::silex::html::main(children).style("padding: 20px 0;"),
        footer!(p("Built with Silex & Rust")).style(
            "margin-top: 50px; padding-top: 20px; border-top: 1px solid #eee; text-align: center; color: #999; font-size: 0.8rem;",
        ),
    )
    .style("font-family: sans-serif; max-width: 800px; margin: 0 auto; color: #333;")
}

// ==========================================
// App 入口
// ==========================================

#[component]
fn App<'scope>(scope: Scope<'scope>, error_handler: ErrorReporter<'scope>) -> impl View<'scope> {
    let users = routes!(UsersRoutes {
        list "/" => move |ctx| UserList(ctx).build(),
        create "/new" => move |_ctx| CreateUser().build(),
        detail "/:id" => move |ctx, id: u32| UserDetail(ctx, id).build(),
    })
    .at("/users");
    let routes = routes!(AppRoutes {
        home "/" => move |_ctx| Home().build(),
        search "/search" => move |ctx| SearchPage(ctx).error_handler(error_handler).build(),
        not_found "/*" => move |ctx| NotFound(ctx).build(),
    });

    let home_path = routes.home();
    let users_path = users.list();
    let search_path = routes.search();
    let table = routes
        .table()
        .nest(users.prefix(), users.table(), move |ctx, outlet| {
            UsersLayout(ctx).children(outlet).build()
        });

    Router(scope).routes(table).layout(move |ctx, outlet| {
        MainLayout(
            ctx,
            home_path.clone(),
            users_path.clone(),
            search_path.clone(),
        )
        .children(outlet)
        .build()
    })
}

/// Mount the Router demo into the conventional `#app` target.
pub fn mount_router() -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::from_id("app")?;
    inject_router_styles();
    bootstrap.mount(Runtime::new(), mount_router_view)?;
    bootstrap.into_js_host()
}

/// Mount the Router demo into a caller-provided target node.
pub fn mount_router_into(target: web_sys::Node) -> Result<JsAppHost, BootstrapError> {
    let mut bootstrap = BrowserBootstrap::new(target);
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
    });
    context.mount(App(scope, error_handler).build(), error_handler)
}
