use silex::prelude::*;
use silex::reexports::*;

// 1. 定义数据模型
#[derive(Clone, Debug, Store)]
struct User {
    name: String,
    age: i32,
    email: String,
}

fn main() {
    setup_global_error_handlers();
    let window = web_sys::window().expect("No Window");
    let document = window.document().expect("No Document");
    let app_container = document.get_element_by_id("app").expect("No App Element");

    create_scope(move || {
        // 2. 初始化 Store
        // 宏会自动生成 UserStore 结构体和 new 方法
        let user = UserStore::new(User {
            name: "Alice".to_string(),
            age: 25,
            email: "alice@example.com".to_string(),
        });

        // 3. 构建 UI
        App().user(user).mount(&app_container);
    });
}

// 使用 #[component] 宏定义带参数的组件
#[component]
fn App(user: UserStore) -> impl View {
    div((
        h1("Silex Store Demo"),
        p("This example demonstrates fine-grained reactivity using the #[derive(Store)] macro."),
        
        // 显示区域
        UserDisplay().user(user.clone()),

        // 编辑区域
        UserEditor().user(user.clone()),
        
        // 调试信息：展示 Store 导出功能
        DebugPanel().user(user)
    ))
    .style("padding: 20px; font-family: sans-serif; max-width: 500px; margin: 0 auto; border: 1px solid #ccc; border-radius: 8px;")
}

// 用户信息显示组件
#[component]
fn UserDisplay(user: UserStore) -> impl View {
    div((
        div((
            span("Name: ").style("font-weight: bold;"),
            // 直接绑定 store.name (ReadSignal)
            // 修改 age 不会触发这个文本节点的更新
            span(user.name.clone()),
        )),
        div((
            span("Age: ").style("font-weight: bold;"),
            span(move || user.age.get().to_string()),
        )),
        div((
            span("Email: ").style("font-weight: bold;"),
            span(user.email.clone()),
        )),
    ))
    .style("background: #f5f5f5; padding: 15px; border-radius: 4px; margin-bottom: 20px;")
}

// 用户编辑组件
#[component]
fn UserEditor(user: UserStore) -> impl View {
    div((
        // 修改 Name
        div((
            label("Change Name: "),
            input()
                .attr("type", "text")
                .attr("value", user.name.clone())
                .on_input(move |new_val| user.name.set(new_val)),
        )),
        // 修改 Age
        div((
            label("Change Age: "),
            button("Increment Age").on_click(move |_| {
                user.age.update(|age| *age += 1);
            }),
            span("(Only updates Age node)").style("margin-left: 10px; color: #666;"),
        )),
        // 修改 Email
        div((
            label("Change Email: "),
            input()
                .attr("type", "text")
                .attr("value", user.email.clone())
                .on_input(move |new_val| user.email.set(new_val)),
        )),
    ))
    .style("display: flex; flex-direction: column; gap: 10px;")
}

// 调试面板组件
#[component]
fn DebugPanel(user: UserStore) -> impl View {
    div((button("Log Current State to Console").on_click(move |_| {
        // 演示 get() 方法还原普通结构体
        let current_state = user.get();
        web_sys::console::log_1(&format!("Current Store State: {:?}", current_state).into());
    }),))
    .style("margin-top: 20px; border-top: 1px dashed #ccc; padding-top: 10px;")
}
