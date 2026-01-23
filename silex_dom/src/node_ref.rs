use std::cell::RefCell;
use std::rc::Rc;

/// `NodeRef` 用于获取对底层 DOM 节点的直接引用。
///
/// 这在需要使用命令式 DOM API（如 `.focus()`, `.show_modal()`, Canvas 绘图等）时非常有用。
///
/// # 示例
///
/// ```rust,no_run
/// use silex::prelude::*;
/// use web_sys::HtmlInputElement;
///
/// let input_ref = NodeRef::<HtmlInputElement>::new();
///
/// input()
///     .node_ref(input_ref)
///     .on_mount(move |_| {
///         if let Some(el) = input_ref.get() {
///             el.focus().unwrap();
///         }
///     });
/// ```
#[derive(Clone, Default)]
pub struct NodeRef<T = web_sys::Element>(Rc<RefCell<Option<T>>>);

impl<T> NodeRef<T> {
    /// 创建一个新的空 `NodeRef`。
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    /// 获取存储的节点引用。如果节点尚未加载，返回 `None`。
    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.0.borrow().clone()
    }

    /// 加载（设置）节点引用。通常由框架内部调用。
    pub fn load(&self, node: T) {
        *self.0.borrow_mut() = Some(node);
    }
}

impl<T: 'static> std::fmt::Debug for NodeRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeRef({:?})", self.0.as_ptr())
    }
}
