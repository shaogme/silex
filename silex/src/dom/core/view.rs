use crate::dom::element::Element;
use crate::reactivity::{ReadSignal, RwSignal, create_effect};
use crate::{SilexError, SilexResult};
use std::fmt::Display;
use web_sys::Node;

/// 视图特征 (View Trait)
/// 核心特征：定义了如何将一个东西挂载到 DOM 上。
pub trait View {
    fn mount(self, parent: &Node);
}

// --- View Trait Implementations ---

// 1. Element 本身就是 View
impl View for Element {
    fn mount(self, parent: &Node) {
        if let Err(e) = parent
            .append_child(&self.dom_element)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
        }
    }
}

// 2. 静态文本 (String, &str)
impl View for String {
    fn mount(self, parent: &Node) {
        let document = crate::dom::document();
        let node = document.create_text_node(&self);
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            crate::error::handle_error(e);
        }
    }
}

impl View for &str {
    fn mount(self, parent: &Node) {
        let document = crate::dom::document();
        let node = document.create_text_node(self);
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            crate::error::handle_error(e);
        }
    }
}

// 3. 基础类型支持
macro_rules! impl_view_for_primitive {
    ($($t:ty),*) => {
        $(
            impl View for $t {
                fn mount(self, parent: &Node) {
                    let document = crate::dom::document();
                    let node = document.create_text_node(&self.to_string());
                    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
                        crate::error::handle_error(e);
                    }
                }
            }
        )*
    };
}

impl_view_for_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

// 4. 动态闭包支持 (Lazy View / Dynamic Text)
impl<F, V> View for F
where
    F: Fn() -> V + 'static,
    V: View + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::dom::document();

        // 1. 创建锚点 (Comment Node)
        let marker = document.create_comment("dyn-view");
        let marker_node: Node = marker.into();

        if let Err(e) = parent.append_child(&marker_node).map_err(SilexError::from) {
            crate::error::handle_error(e);
            return;
        }

        // 2. 状态追踪：用于记录上一次挂载产生的节点，以便清理
        // 这里使用 Rc<RefCell> 是因为 create_effect 需要 Fn (不可变)，但我们需要修改状态
        use std::cell::RefCell;
        use std::rc::Rc;
        let mounted_nodes = Rc::new(RefCell::new(Vec::<Node>::new()));

        create_effect(move || {
            // 在产生副作用时捕获 Panic，防止整个应用崩溃，并允许 ErrorBoundary 捕获
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let view = self();

                // A. 清理旧节点
                let mut nodes = mounted_nodes.borrow_mut();
                for node in nodes.drain(..) {
                    if let Some(p) = node.parent_node() {
                        let _ = p.remove_child(&node);
                    }
                }

                // B. 准备新内容 (使用 DocumentFragment 收集节点)
                let fragment = document.create_document_fragment();
                let fragment_node: Node = fragment.clone().into();

                // 挂载到 Fragment
                view.mount(&fragment_node);

                // C. 收集新节点引用 (在插入到 DOM 前收集)
                let child_nodes = fragment.child_nodes();
                for i in 0..child_nodes.length() {
                    if let Some(node) = child_nodes.item(i) {
                        nodes.push(node);
                    }
                }

                // D. 插入到 DOM (在锚点之前)
                if let Some(parent) = marker_node.parent_node() {
                    let _ = parent.insert_before(&fragment_node, Some(&marker_node));
                }
            }));

            if let Err(payload) = result {
                // 转换 Panic payload 为 SilexError
                let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                    format!("Panic in View: {}", s)
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    format!("Panic in View: {}", s)
                } else {
                    "Unknown Panic in View".to_string()
                };

                crate::error::handle_error(SilexError::Javascript(msg));
            }
        });
    }
}

// 5. 直接 Signal 支持
impl<T> View for ReadSignal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::dom::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            crate::error::handle_error(e);
            return;
        }

        // 2. 创建副作用
        let signal = self;
        create_effect(move || {
            let value = signal.get();
            node.set_node_value(Some(&value.to_string()));
        });
    }
}

impl<T> View for RwSignal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        self.read_signal().mount(parent);
    }
}

// 6. 容器类型支持
impl<V: View> View for Option<V> {
    fn mount(self, parent: &Node) {
        if let Some(v) = self {
            v.mount(parent);
        }
    }
}

impl<V: View> View for Vec<V> {
    fn mount(self, parent: &Node) {
        for v in self {
            v.mount(parent);
        }
    }
}

// 7. 元组支持
macro_rules! impl_view_for_tuple {
    ($($name:ident),*) => {
        impl<$($name: View),*> View for ($($name,)*) {
            #[allow(non_snake_case)]
            fn mount(self, parent: &Node) {
                let ($($name,)*) = self;
                $($name.mount(parent);)*
            }
        }
    }
}
impl_view_for_tuple!(A);
impl_view_for_tuple!(A, B);
impl_view_for_tuple!(A, B, C);
impl_view_for_tuple!(A, B, C, D);
impl_view_for_tuple!(A, B, C, D, E);
impl_view_for_tuple!(A, B, C, D, E, F);
impl_view_for_tuple!(A, B, C, D, E, F, G);
impl_view_for_tuple!(A, B, C, D, E, F, G, H);
impl_view_for_tuple!(A, B, C, D, E, F, G, H, I);
impl_view_for_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_view_for_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_view_for_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

// 8. Result 支持
impl<V: View> View for SilexResult<V> {
    fn mount(self, parent: &Node) {
        match self {
            Ok(v) => v.mount(parent),
            Err(e) => crate::error::handle_error(e),
        }
    }
}

// --- AnyView (Type Erasure) ---

/// 辅助特征，用于支持 Box<dyn View> 的移动语义挂载
pub trait Render {
    fn mount_boxed(self: Box<Self>, parent: &Node);
}

impl<V: View + 'static> Render for V {
    fn mount_boxed(self: Box<Self>, parent: &Node) {
        (*self).mount(parent)
    }
}

/// 类型擦除的 View，可以持有任何 View 的实现。
/// 用于从同一个函数返回不同类型的 View（例如：主题）。
pub struct AnyView(Box<dyn Render>);

impl AnyView {
    pub fn new<V: View + 'static>(view: V) -> Self {
        Self(Box::new(view))
    }
}

impl View for AnyView {
    fn mount(self, parent: &Node) {
        self.0.mount_boxed(parent)
    }
}

pub trait IntoAnyView {
    fn into_any(self) -> AnyView;
}

impl<V: View + 'static> IntoAnyView for V {
    fn into_any(self) -> AnyView {
        AnyView::new(self)
    }
}
