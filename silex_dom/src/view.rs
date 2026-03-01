pub mod any;
pub mod reactive;

pub use any::*;
pub use reactive::*;

use crate::attribute::PendingAttribute;
use silex_core::error::handle_error;
use silex_core::reactivity::Effect;
use silex_core::{SilexError, SilexResult};
use std::panic::{AssertUnwindSafe, catch_unwind};
use web_sys::Node;

/// 递归视图链辅助结构 - 空节点
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewNil;

/// 递归视图链辅助结构 - 构造节点
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewCons<H, T>(pub H, pub T);

/// 视图特征 (View Trait)
/// 核心特征：定义了如何将一个东西挂载到 DOM 上。
pub trait View {
    /// Mount this view to a parent node with a set of pending attributes.
    /// This is the primary entry point for mounting views.
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>);

    /// Apply forwarded attributes to this view.
    /// Default implementation does nothing (for Text, Fragment, etc.).
    /// Elements override this to actually apply attributes.
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}

    /// Convert this view into an AnyView (Type Erasure without Clone requirement).
    fn into_any(self) -> AnyView
    where
        Self: Sized + 'static,
    {
        AnyView::Unique(Box::new(self), Vec::new())
    }

    /// Convert this view into a SharedView (Type Erasure with Clone requirement).
    fn into_shared(self) -> SharedView
    where
        Self: Sized + Clone + 'static,
    {
        SharedView::SharedBoxed(Box::new(self), Vec::new())
    }
}

/// Non-generic helper to mount a text node. Reduces monomorphization bloat for static text.
pub fn mount_text_node(parent: &Node, text: &str) {
    let document = crate::document();
    let node = document.create_text_node(text);
    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
        handle_error(e);
    }
}

// --- View Trait Implementations for Base Types ---

// 1. 静态文本 (String, &str)
impl View for String {
    fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, &self);
    }

    fn into_any(self) -> AnyView {
        AnyView::Text(self.clone())
    }

    fn into_shared(self) -> SharedView {
        SharedView::Text(self)
    }
}

impl View for &str {
    fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, self);
    }

    fn into_any(self) -> AnyView {
        AnyView::Text(self.to_string())
    }

    fn into_shared(self) -> SharedView {
        SharedView::Text(self.to_string())
    }
}

// 2. 基础类型支持
macro_rules! impl_view_for_primitive {
    ($($t:ty),*) => {
        $(
            impl View for $t {
                fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
                    mount_text_node(parent, &self.to_string());
                }

                fn into_any(self) -> AnyView {
                    AnyView::Text(self.to_string())
                }

                fn into_shared(self) -> SharedView {
                    SharedView::Text(self.to_string())
                }
            }
        )*
    };
}

impl_view_for_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl View for () {
    fn mount(self, _parent: &Node, _attrs: Vec<PendingAttribute>) {}

    fn into_any(self) -> AnyView {
        AnyView::Empty
    }

    fn into_shared(self) -> SharedView {
        SharedView::Empty
    }
}

// 3. 动态闭包支持 (Lazy View / Dynamic Text)
impl<F, V> View for F
where
    F: Fn() -> V + 'static,
    V: View + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_dynamic_view_erased_internal(parent, attrs, Box::new(move || self().into_any()));
    }
}

/// 非泛型的动态视图挂载内核，用于减少单态化膨胀。
fn mount_dynamic_view_erased_internal(
    parent: &Node,
    attrs: Vec<PendingAttribute>,
    producer: Box<dyn Fn() -> AnyView>,
) {
    let document = crate::document();

    // 1. 创建锚点 (Start & End Markers)
    let start_marker = document.create_comment("dyn-start");
    let start_node: Node = start_marker.into();

    if let Err(e) = parent.append_child(&start_node).map_err(SilexError::from) {
        handle_error(e);
        return;
    }

    let end_marker = document.create_comment("dyn-end");
    let end_node: Node = end_marker.into();

    if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
        handle_error(e);
        return;
    }

    use std::cell::Cell;
    use std::rc::Rc;
    let prev_scope = Rc::new(Cell::new(None::<silex_core::reactivity::NodeId>));

    Effect::new(move |_| {
        if let Some(id) = prev_scope.get() {
            silex_core::reactivity::dispose(id);
            prev_scope.set(None);
        }

        let start_node = start_node.clone();
        let end_node = end_node.clone();
        let document = document.clone();
        let attrs = attrs.clone();

        let result = catch_unwind(AssertUnwindSafe(|| {
            let view = producer();

            let start_node = start_node.clone();
            let end_node = end_node.clone();
            let document = document.clone();
            let id = silex_core::reactivity::create_scope(move || {
                if let Some(parent) = start_node.parent_node() {
                    while let Some(sibling) = start_node.next_sibling() {
                        if sibling == end_node {
                            break;
                        }
                        let _ = parent.remove_child(&sibling);
                    }
                }

                let fragment = document.create_document_fragment();
                let fragment_node: Node = fragment.clone().into();

                view.mount(&fragment_node, attrs);

                if let Some(parent) = end_node.parent_node() {
                    let _ = parent.insert_before(&fragment_node, Some(&end_node));
                }
            });
            id
        }));

        if let Ok(id) = result {
            prev_scope.set(Some(id));
        } else if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                format!("Panic in View: {}", s)
            } else if let Some(s) = payload.downcast_ref::<String>() {
                format!("Panic in View: {}", s)
            } else {
                "Unknown Panic in View".to_string()
            };

            handle_error(SilexError::Javascript(msg));
        }
    });
}

// 3.6 Type closure delegation
impl<V> View for std::rc::Rc<dyn Fn() -> V>
where
    V: View + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let f = self.clone();
        (move || f()).mount(parent, attrs);
    }
}

// 5. 容器类型支持
impl<V: View> View for Option<V> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        if let Some(v) = self {
            v.mount(parent, attrs);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        if let Some(v) = self {
            v.apply_attributes(attrs);
        }
    }
}

impl<V: View> View for Vec<V> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, v) in self.into_iter().enumerate() {
            v.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for v in self {
            v.apply_attributes(attrs.clone());
        }
    }
}

impl<V: View, const N: usize> View for [V; N] {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, v) in self.into_iter().enumerate() {
            v.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for v in self {
            v.apply_attributes(attrs.clone());
        }
    }
}

// 6. 递归元组支持 (Recursive Tuple Support)

impl View for ViewNil {
    fn mount(self, _parent: &Node, _attrs: Vec<PendingAttribute>) {}
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}

    fn into_any(self) -> AnyView {
        AnyView::Empty
    }

    fn into_shared(self) -> SharedView {
        SharedView::Empty
    }
}

impl<H: View, T: View> View for ViewCons<H, T> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        // 头节点接收 attributes
        self.0.mount(parent, attrs);
        // 后续链表不再接受 attributes (避免重复应用)
        self.1.mount(parent, Vec::new());
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

/// 将多个视图链接成递归视图链的宏。
///
/// 用于替代标准元组以减少单态化膨胀。
#[macro_export]
macro_rules! view_chain {
    () => {
        $crate::view::ViewNil
    };
    ($head:expr $(,)?) => {
        $crate::view::ViewCons($head, $crate::view::ViewNil)
    };
    ($head:expr, $($tail:expr),+ $(,)?) => {
        $crate::view::ViewCons($head, $crate::view_chain!($($tail),+))
    };
}

// 7. Result 支持
impl<V: View> View for SilexResult<V> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            Ok(v) => v.mount(parent, attrs),
            Err(e) => handle_error(e),
        }
    }
}
