use silex_core::reactivity::on_cleanup;
use silex_dom::prelude::View;
use web_sys::Node;

/// Portal 组件：将子视图渲染到当前 DOM 树之外的节点（默认是 document.body）。
/// 但保持响应式上下文（Context）的连通性。
#[derive(Clone)]
pub struct Portal<V> {
    child: V,
    mount_element: Option<Node>,
}

impl<V> Portal<V> {
    pub fn new(child: V) -> Self {
        Self {
            child,
            mount_element: None,
        }
    }

    /// 指定挂载的目标节点。
    pub fn mount_to(mut self, element: Node) -> Self {
        self.mount_element = Some(element);
        self
    }
}

impl<V> View for Portal<V>
where
    V: View,
{
    fn mount(self, _parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        let document = silex_dom::document();
        let target = self
            .mount_element
            .unwrap_or_else(|| document.body().expect("Body not found").into());

        let container = document
            .create_element("div")
            .expect("Failed to create portal container");
        let _ = container.set_attribute("style", "display: contents");
        let container_node: Node = container.into();

        let _ = target.append_child(&container_node);

        // Movement is allowed here as 'self' is owned
        self.child.mount(&container_node, attrs);

        let target_clone = target.clone();
        let container_clone = container_node.clone();
        on_cleanup(move || {
            let _ = target_clone.remove_child(&container_clone);
        });
    }

    fn mount_ref(&self, _parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        let document = silex_dom::document();
        let target = self
            .mount_element
            .clone()
            .unwrap_or_else(|| document.body().expect("Body not found").into());

        let container = document
            .create_element("div")
            .expect("Failed to create portal container");
        let _ = container.set_attribute("style", "display: contents");
        let container_node: Node = container.into();

        let _ = target.append_child(&container_node);

        self.child.mount_ref(&container_node, attrs);

        let target_clone = target.clone();
        let container_clone = container_node.clone();
        on_cleanup(move || {
            let _ = target_clone.remove_child(&container_clone);
        });
    }
}

pub fn portal<V: View>(child: V) -> Portal<V> {
    Portal::new(child)
}
