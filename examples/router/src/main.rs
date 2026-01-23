use silex::prelude::*;

// ==========================================
// 辅助组件
// ==========================================

/// 一个简单的卡片容器
#[component]
fn Card<V: View + 'static>(child: V) -> impl View {
    div(child)
        .style("border: 1px solid #ddd; border-radius: 8px; padding: 20px; margin: 10px 0; background: white; box-shadow: 0 2px 4px rgba(0,0,0,0.05);")
}

/// 导航链接样式封装
fn nav_link<T: ToRoute>(to: T, label: &str) -> impl View {
    Link(to)
        .text(label)
        .style("margin-right: 15px; text-decoration: none; color: #666; padding: 5px 10px; border-radius: 4px; transition: all 0.2s;")
        .active_class("nav-active") // 需要在全局 CSS 中定义 .nav-active { background: #e3f2fd; color: #1976d2; font-weight: bold; }
}

// ==========================================
// 页面组件
// ==========================================

#[component]
fn Home() -> impl View {
    div((
        h2("🏠 Home Page"),
        p("Welcome to the Router Test Suite."),
        p("Use the navigation bar above to test different routing features."),
    ))
}

#[component]
fn SearchPage() -> impl View {
    // 测试查询参数 hooks：使用 use_query_signal 实现双向绑定
    // 只要改变 search_term，URL 就会更新；URL 变了，search_term 也会更新
    let search_term = use_query_signal("q");
    let display_term = search_term.clone(); // 用于展示

    Card().child(div((
        h2("🔍 Search Query Test"),
        p("Type in the input below. The URL query parameter 'q' will update automatically!"),
        div((
            input()
                .type_("text")
                .placeholder("Type search term...")
                .bind_value(search_term) // 双向绑定到 RwSignal
                .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; flex: 1;"),
            button("Clear")
                .on_click(move |_| search_term.set(String::new()))
                .style("padding: 8px 16px; background: #f44336; color: white; border: none; border-radius: 4px; cursor: pointer;"),
        )).style("display: flex; gap: 10px; margin-bottom: 20px;"),
        div((
            strong("Current Query Parameter (q): "),
            span(move || {
                let v = display_term.get();
                if v.is_empty() { "None".to_string() } else { v }
            }).style("color: #e91e63; font-family: monospace;")
        ))
    )))
}

// --- 用户模块 (嵌套路由测试) ---

#[component]
fn CreateUser() -> impl View {
    Card().child(h3("🆕 Create New User Form"))
}

#[component]
fn UsersLayout(route: UsersRoute) -> impl View {
    div((
        h2("👥 Users Module"),
        div((
            nav_link("/users", "User List"),
            span("|").style("margin: 0 10px; color: #ccc;"),
            nav_link("/users/new", "Create User (Static)"),
        )).style("border-bottom: 2px solid #eee; padding-bottom: 10px; margin-bottom: 20px;"),
        // 渲染子路由
        route.render()
    ))
}

#[component]
fn UserList() -> impl View {
    let users = vec![
        (1, "Alice"),
        (2, "Bob"),
        (3, "Charlie"),
        (42, "Silex Expert"),
    ];

    div((
        h3("Select a User:"),
        ul(
            users.into_iter().map(|(id, name)| {
                li(
                    Link(UsersRoute::Detail { id })
                        .text(&format!("👤 {} (ID: {})", name, id))
                        .style("text-decoration: none; color: #2196f3;")
                        .active_class("active-user")
                ).style("margin: 5px 0;")
            }).collect::<Vec<_>>()
        ).style("list-style: none; padding: 0;")
    ))
}

#[component]
fn UserDetail(id: u32) -> impl View {
    // 使用传入的 id，不再依赖 use_params (更类型安全!)
    let navigator = use_navigate();
    let path = use_location_path();

    Card().child(div((
        div((
            h3(format!("User Profile: #{}", id)),
            button("Go Back")
                .on_click(move |_| navigator.push(AppRoute::Users { route: UsersRoute::List }))
                .style("font-size: 0.8rem; padding: 5px 10px; cursor: pointer;")
        )).style("display: flex; justify-content: space-between; align-items: center;"),
        hr().style("border: 0; border-top: 1px solid #eee; margin: 15px 0;"),
        p((strong("Current Path: "), span(path).style("font-family: monospace;"))),
        div(
            p(format!("This component is rendered with strict prop id: {}", id))
        ).style("background: #f5f5f5; padding: 10px; border-radius: 4px; margin-top: 10px;")
    )))
}

#[component]
fn NotFound() -> impl View {
    div((
        h1("404"),
        p("Page not found."),
        Link("/").text("Return Home").style("color: #2196f3; text-decoration: underline;")
    ))
    .style("text-align: center; padding: 50px; color: #d32f2f;")
}

// --- 主布局 ---

#[component]
fn MainLayout(child: AnyView) -> impl View {
    div((
        // Header
        header((
            h1("🚀 Silex Router").style("margin: 0; font-size: 1.5rem; color: #2c3e50;"),
            nav((
                nav_link(AppRoute::Home, "Home"),
                nav_link("/users", "Users"), // 混合使用：字符串仍然有效
                nav_link(AppRoute::Search, "Search"),
                nav_link("/nowhere", "404 Test"),
            ))
        ))
        .style("display: flex; align-items: center; justify-content: space-between; padding: 20px 0; border-bottom: 1px solid #eee;"),
        
        // Main Content Area
        silex::prelude::main(
            child
        ).style("padding: 20px 0;"),

        // Footer
        footer(
            p("Built with Silex & Rust")
        ).style("margin-top: 50px; padding-top: 20px; border-top: 1px solid #eee; text-align: center; color: #999; font-size: 0.8rem;")
    ))
    .style("font-family: sans-serif; max-width: 800px; margin: 0 auto; color: #333;")
}


// 定义子路由枚举 (Users Module)
#[derive(Route, Clone, PartialEq)]
enum UsersRoute {
    #[route("/", view = UserListComponent)]
    List,
    #[route("/new", view = CreateUserComponent)]
    Create,
    #[route("/:id", view = UserDetailComponent)]
    Detail { id: u32 },
}

// 定义应用顶级路由枚举
#[derive(Route, Clone, PartialEq)]
enum AppRoute {
    #[route("/", view = HomeComponent)]
    Home,
    #[route("/search", view = SearchPageComponent)]
    Search,
    
    // 递归嵌套：所有以 /users 开头的路径交给 UsersRoute 处理
    #[route("/users/*", view = UsersLayoutComponent)]
    Users {
        #[nested]
        route: UsersRoute 
    },

    #[route("/*", view = NotFoundComponent)]
    NotFound,
}

// ==========================================
// App 入口
// ==========================================

fn main() {
    setup_global_error_handlers();
    
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    // 注入全局样式用于 Active Link 高亮
    let style_el = document.create_element("style").unwrap();
    style_el.set_text_content(Some(".nav-active { background-color: #e3f2fd !important; color: #1976d2 !important; font-weight: bold; }"));
    
    if let Ok(Some(head)) = document.query_selector("head") {
        let _ = head.append_child(&style_el);
    } else {
        let _ = body.append_child(&style_el);
    }

    // 创建一个渲染闭包，将路由映射到视图
    // 采用“视图组合”模式：match 分发 + Layout 函数包裹
    
    create_scope(move || {
        let app = MainLayout().child(
            Router::new()
                .match_route::<AppRoute>()
        );
        app.mount(&body);
    });
}
