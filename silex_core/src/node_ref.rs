use std::marker::PhantomData;

pub use silex_reactivity::NodeId;

/// `NodeRef` 用于获取对底层 DOM 节点的直接引用。
///
/// 此类型使用 `NodeId` 句柄引用存储在响应式运行时中的元素，
/// 实现了 `Copy` 语义，与 `Signal` 和 `Memo` 风格一致。
///
/// 这在需要使用命令式 DOM API（如 `.focus()`, `.show_modal()`, Canvas 绘图等）时非常有用。
///
/// # 示例
///
/// ```rust,ignore
/// use silex::prelude::*;
/// use web_sys::HtmlInputElement;
///
/// let input_ref = NodeRef::<HtmlInputElement>::new();
///
/// input()
///     .node_ref(input_ref)  // 无需 clone，NodeRef 是 Copy 的
///     .on_mount(move |_| {
///         if let Some(el) = input_ref.get() {
///             el.focus().unwrap();
///         }
///     });
/// ```
#[derive(Debug)]
pub struct NodeRef<T = ()> {
    id: NodeId,
    marker: PhantomData<T>,
}

impl<T> Clone for NodeRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for NodeRef<T> {}

impl<T: Clone + 'static> Default for NodeRef<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + 'static> NodeRef<T> {
    /// 创建一个新的空 `NodeRef`。
    pub fn new() -> Self {
        let id = silex_reactivity::register_node_ref();
        Self {
            id,
            marker: PhantomData,
        }
    }

    /// 获取存储的节点引用。如果节点尚未加载，返回 `None`。
    pub fn get(&self) -> Option<T> {
        silex_reactivity::get_node_ref(self.id)
    }

    /// 加载（设置）节点引用。通常由框架内部调用。
    pub fn load(&self, node: T) {
        silex_reactivity::set_node_ref(self.id, node);
    }

    /// 返回此 `NodeRef` 的底层 `NodeId`。
    pub fn id(&self) -> NodeId {
        self.id
    }
}
