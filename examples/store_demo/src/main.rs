use silex::prelude::*;
use silex::dom::tag::*;
use silex::Store;

// 1. 定义数据模型
#[derive(Clone, Debug, Store)]
struct User {
    name: String,
    age: i32,
    email: String,
}

fn main() {
    silex::dom::setup_global_error_handlers();
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
        App::new(user).mount(&app_container);
    });
}

// 使用 #[component] 宏定义带参数的组件
#[component]
fn App(user: UserStore) -> impl View {
    div()
        .style("padding: 20px; font-family: sans-serif; max-width: 500px; margin: 0 auto; border: 1px solid #ccc; border-radius: 8px;")
        .child((
            h1().text("Silex Store Demo"),
            p().text("This example demonstrates fine-grained reactivity using the #[derive(Store)] macro."),
            
            // 显示区域
            UserDisplay::new(user.clone()),

            // 编辑区域
            UserEditor::new(user.clone()),
            
            // 调试信息：展示 Store 导出功能
            DebugPanel::new(user)
        ))
}

// 用户信息显示组件
#[component]
fn UserDisplay(user: UserStore) -> impl View {
    div().style("background: #f5f5f5; padding: 15px; border-radius: 4px; margin-bottom: 20px;").child((
        div().child((
            span().style("font-weight: bold;").text("Name: "),
            // 直接绑定 store.name (ReadSignal)
            // 修改 age 不会触发这个文本节点的更新
            span().text(user.name.clone())
        )),
        div().child((
            span().style("font-weight: bold;").text("Age: "),
            span().text(move || user.age.get().to_string())
        )),
        div().child((
            span().style("font-weight: bold;").text("Email: "),
            span().text(user.email.clone())
        )),
    ))
}

// 用户编辑组件
#[component]
fn UserEditor(user: UserStore) -> impl View {
    div().style("display: flex; flex-direction: column; gap: 10px;").child((
        // 修改 Name
        div().child((
            label().text("Change Name: "),
            input()
                .attr("type", "text")
                .attr("value", user.name.clone())
                .on_input(move |new_val| user.name.set(new_val))
        )),
        
        // 修改 Age
        div().child((
            label().text("Change Age: "),
            button()
                .text("Increment Age")
                .on_click(move |_| {
                    user.age.update(|age| *age += 1);
                }),
            span().style("margin-left: 10px; color: #666;").text("(Only updates Age node)")
        )),

        // 修改 Email
        div().child((
            label().text("Change Email: "),
            input()
                .attr("type", "text")
                .attr("value", user.email.clone())
                .on_input(move |new_val| user.email.set(new_val))
        )),
    ))
}

// 调试面板组件
#[component]
fn DebugPanel(user: UserStore) -> impl View {
    div().style("margin-top: 20px; border-top: 1px dashed #ccc; padding-top: 10px;").child((
        button()
            .text("Log Current State to Console")
            .on_click(move |_| {
                // 演示 get() 方法还原普通结构体
                let current_state = user.get();
                web_sys::console::log_1(&format!("Current Store State: {:?}", current_state).into());
            }),
    ))
}
