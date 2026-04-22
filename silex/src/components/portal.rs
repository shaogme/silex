use silex_core::reactivity::on_cleanup;
use silex_dom::prelude::*;
use silex_macros::component;
use web_sys::Node;

/// Portal 组件：将子视图渲染到当前 DOM 树之外的节点（默认是 document.body）。
/// 但保持响应式上下文（Context）的连通性。
#[component]
pub fn Portal(
    #[prop(into)] children: AnyView,
    #[prop(default)] mount_to: Option<Node>,
) -> impl View {
    let document = silex_dom::document();
    let target = mount_to
        .clone()
        .unwrap_or_else(|| document.body().expect("Body not found").into());

    let container = document
        .create_element("div")
        .expect("Failed to create portal container");
    let _ = container.set_attribute("style", "display: contents");
    let container_node: Node = container.into();

    let _ = target.append_child(&container_node);

    let target_clone = target.clone();
    let container_clone = container_node.clone();
    on_cleanup(move || {
        let _ = target_clone.remove_child(&container_clone);
    });

    children.mount_owned(&container_node, Vec::new());

    // 返回空视图，因为 Portal 在原位置不渲染内容
}
