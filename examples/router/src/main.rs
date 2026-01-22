use silex::prelude::*;
use silex::dom::tag::*;
use silex::router::{Router, Route, Outlet, link, use_navigate, use_params, use_query_map, use_location_path};

// ==========================================
// 辅助组件
// ==========================================

/// 一个简单的卡片容器
#[component]
fn Card<V: View + 'static>(child: V) -> impl View {
    div()
        .style("border: 1px solid #ddd; border-radius: 8px; padding: 20px; margin: 10px 0; background: white; box-shadow: 0 2px 4px rgba(0,0,0,0.05);")
        .child(child)
}

/// 导航链接样式封装
fn nav_link(to: &str, label: &str) -> impl View {
    link(to)
        .text(label)
        .style("margin-right: 15px; text-decoration: none; color: #666; padding: 5px 10px; border-radius: 4px; transition: all 0.2s;")
        .active_class("nav-active") // 需要在全局 CSS 中定义 .nav-active { background: #e3f2fd; color: #1976d2; font-weight: bold; }
}

// ==========================================
// 页面组件
// ==========================================

#[component]
fn Home() -> impl View {
    div().child((
        h2().text("🏠 Home Page"),
        p().text("Welcome to the Router Test Suite."),
        p().text("Use the navigation bar above to test different routing features."),
    ))
}

#[component]
fn SearchPage() -> impl View {
    // 测试查询参数 hooks
    let query = use_query_map();
    let navigator = use_navigate();
    let (input_val, set_input_val) = create_signal(String::new());

    // 当 URL 变化时，同步 input 的值
    create_effect(move || {
        if let Some(q) = query.get().get("q") {
            set_input_val.set(q.clone());
        }
    });

    let on_search = move |_| {
        let val = input_val.get();
        if !val.is_empty() {
            // 编程式导航：推入带查询参数的 URL
            navigator.push(&format!("/search?q={}", val));
        } else {
            navigator.push("/search");
        }
    };

    Card::new(div().child((
        h2().text("🔍 Search Query Test"),
        div().style("display: flex; gap: 10px; margin-bottom: 20px;").child((
            input()
                .attr("type", "text")
                .attr("placeholder", "Type search term...")
                .attr("value", input_val)
                .on_input(move |v| set_input_val.set(v))
                .style("padding: 8px; border: 1px solid #ccc; border-radius: 4px; flex: 1;"),
            button()
                .text("Search")
                .on_click(on_search)
                .style("padding: 8px 16px; background: #2196f3; color: white; border: none; border-radius: 4px; cursor: pointer;"),
        )),
        div().child((
            strong().text("Current Query Parameter (q): "),
            span().style("color: #e91e63; font-family: monospace;").text(move || {
                query.get().get("q").cloned().unwrap_or_else(|| "None".to_string())
            })
        ))
    )))
}

// --- 用户模块 (嵌套路由测试) ---

#[component]
fn UsersLayout() -> impl View {
    div().child((
        h2().text("👥 Users Module"),
        div().style("border-bottom: 2px solid #eee; padding-bottom: 10px; margin-bottom: 20px;").child((
            nav_link("/users", "User List"),
            span().text("|").style("margin: 0 10px; color: #ccc;"),
            nav_link("/users/new", "Create User (Static)"),
        )),
        // 渲染子路由 (UserList 或 UserDetail)
        Outlet()
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

    div().child((
        h3().text("Select a User:"),
        ul().style("list-style: none; padding: 0;").child(
            users.into_iter().map(|(id, name)| {
                li().style("margin: 5px 0;").child(
                    link(&format!("/users/{}", id))
                        .text(&format!("👤 {} (ID: {})", name, id))
                        .style("text-decoration: none; color: #2196f3;")
                        .active_class("active-user")
                )
            }).collect::<Vec<_>>()
        )
    ))
}

#[component]
fn UserDetail() -> impl View {
    // 测试路由参数 hooks
    let params = use_params();
    let navigator = use_navigate();
    let path = use_location_path();

    let user_id = create_memo(move || {
        params.get().get("id").cloned().unwrap_or_else(|| "Unknown".to_string())
    });

    Card::new(div().child((
        div().style("display: flex; justify-content: space-between; align-items: center;").child((
            h3().text(move || format!("User Profile: #{}", user_id.get())),
            button()
                .text("Go Back")
                .on_click(move |_| navigator.push("/users"))
                .style("font-size: 0.8rem; padding: 5px 10px; cursor: pointer;")
        )),
        hr().style("border: 0; border-top: 1px solid #eee; margin: 15px 0;"),
        p().child((strong().text("Current Path: "), span().style("font-family: monospace;").text(path))),
        p().child((strong().text("Raw Params: "), span().style("font-family: monospace; color: #666;").text(move || format!("{:?}", params.get())))),
        div().style("background: #f5f5f5; padding: 10px; border-radius: 4px; margin-top: 10px;").child(
            p().text("This component is rendered because the route matched '/users/:id'")
        )
    )))
}

#[component]
fn NotFound() -> impl View {
    div()
        .style("text-align: center; padding: 50px; color: #d32f2f;")
        .child((
            h1().text("404"),
            p().text("Page not found."),
            link("/").text("Return Home").style("color: #2196f3; text-decoration: underline;")
        ))
}

// --- 主布局 ---

#[component]
fn MainLayout() -> impl View {
    div()
        .style("font-family: sans-serif; max-width: 800px; margin: 0 auto; color: #333;")
        .child((
            // Header
            header()
                .style("display: flex; align-items: center; justify-content: space-between; padding: 20px 0; border-bottom: 1px solid #eee;")
                .child((
                    h1().text("🚀 Silex Router").style("margin: 0; font-size: 1.5rem; color: #2c3e50;"),
                    nav().child((
                        nav_link("/", "Home"),
                        nav_link("/users", "Users"),
                        nav_link("/search", "Search"),
                        nav_link("/nowhere", "404 Test"),
                    ))
                )),
            
            // Main Content Area (Renders matched child route)
            // Explicitly call silex::dom::tag::main because fn main() shadows it
            silex::dom::tag::main().style("padding: 20px 0;").child(
                Outlet()
            ),

            // Footer
            footer()
                .style("margin-top: 50px; padding-top: 20px; border-top: 1px solid #eee; text-align: center; color: #999; font-size: 0.8rem;")
                .child(p().text("Built with Silex & Rust"))
        ))
}

// ==========================================
// App 入口
// ==========================================

fn main() {
    console_error_panic_hook::set_once();
    
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    // 注入全局样式用于 Active Link 高亮
    let style_el = document.create_element("style").unwrap();
    style_el.set_text_content(Some(".nav-active { background-color: #e3f2fd !important; color: #1976d2 !important; font-weight: bold; }"));
    
    // Attempt to append to head, otherwise body (if head is missing in web-sys features)
    if let Ok(Some(head)) = document.query_selector("head") {
        let _ = head.append_child(&style_el);
    } else {
        let _ = body.append_child(&style_el);
    }

    // 定义路由树
    // / -> MainLayout
    //    / -> Home
    //    /search -> SearchPage
    //    /users -> UsersLayout
    //        / -> UserList
    //        /:id -> UserDetail
    //    /* -> NotFound
    // Note: Passing StructName::new function pointers instead of struct types
    let app_routes = Router::new()
        .add(
            Route::new("/", MainLayout::new) // 根布局，包含导航栏
                .children(vec![
                    Route::new("/", Home::new), // 默认子路由
                    Route::new("/search", SearchPage::new),
                    
                    // 嵌套路由模块
                    Route::new("/users", UsersLayout::new)
                        .children(vec![
                            Route::new("/", UserList::new),
                            // 注意：静态路由 "/new" 需要放在动态参数 ":id" 之前，或者依赖路由器的匹配优先级逻辑
                            // 这里 matcher.rs 的实现是顺序匹配或特定逻辑，
                            // 通常建议把具体路径放在参数路径之前，或者使用更智能的匹配器。
                            // 在当前 matcher.rs 中，:id 匹配单段，如果定义了 /users/new 且在 /users/:id 之前 add，应该能匹配。
                            // 但在这里我们是在 children vec 中。
                            // 让我们添加一个静态路由测试：
                            Route::new("/new", || Card::new(h3().text("🆕 Create New User Form"))),
                            Route::new("/:id", UserDetail::new),
                        ]),

                    // 捕获所有其他路径 (在 Layout 内部显示 404，保留导航栏)
                    Route::new("/*", NotFound::new),
                ])
        );

    // 挂载应用
    create_scope(move || {
        app_routes.mount(&body);
    });
}