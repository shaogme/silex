use crate::SilexError;
use silex_core::reactivity::on_cleanup;
use silex_dom::View;
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
    fn mount(self, _parent: &Node) {
        let document = silex_dom::document();
        // 默认挂载到 body
        let target = self
            .mount_element
            .unwrap_or_else(|| document.body().expect("Body not found").into());

        // 创建一个非侵入式的容器
        let container = match document.create_element("div") {
            Ok(el) => el,
            Err(e) => {
                silex_core::error::handle_error(SilexError::from(e));
                return;
            }
        };

        if let Err(e) = container.set_attribute("style", "display: contents") {
            silex_core::error::handle_error(SilexError::from(e));
        }

        let container_node: Node = container.into();

        if let Err(e) = target.append_child(&container_node) {
            silex_core::error::handle_error(SilexError::from(e));
            return;
        }

        // 挂载子元素到容器中
        self.child.mount(&container_node);

        // 注册清理回调：当当前 Scope 被销毁时，移除容器节点
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
