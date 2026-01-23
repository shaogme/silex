use crate::element::Element;
use silex_core::reactivity::{ReadSignal, RwSignal, create_effect};
use silex_core::{SilexError, SilexResult};
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
            silex_core::error::handle_error(e);
        }
    }
}

impl<T> View for crate::element::TypedElement<T> {
    fn mount(self, parent: &Node) {
        if let Err(e) = parent.append_child(&self.element).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
        }
    }
}

// 2. 静态文本 (String, &str)
impl View for String {
    fn mount(self, parent: &Node) {
        let document = crate::document();
        let node = document.create_text_node(&self);
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
        }
    }
}

impl View for &str {
    fn mount(self, parent: &Node) {
        let document = crate::document();
        let node = document.create_text_node(self);
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
        }
    }
}

// 3. 基础类型支持
macro_rules! impl_view_for_primitive {
    ($($t:ty),*) => {
        $(
            impl View for $t {
                fn mount(self, parent: &Node) {
                    let document = crate::document();
                    let node = document.create_text_node(&self.to_string());
                    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
                        silex_core::error::handle_error(e);
                    }
                }
            }
        )*
    };
}

impl_view_for_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl View for () {
    fn mount(self, _parent: &Node) {}
}

// 4. 动态闭包支持 (Lazy View / Dynamic Text)
impl<F, V> View for F
where
    F: Fn() -> V + 'static,
    V: View + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::document();

        // 1. 创建锚点 (Start & End Markers)
        // 使用双锚点策略 (Range Cleaning)，确保清理时能够移除所有动态生成的兄弟节点
        let start_marker = document.create_comment("dyn-start");
        let start_node: Node = start_marker.into();

        if let Err(e) = parent.append_child(&start_node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
            return;
        }

        let end_marker = document.create_comment("dyn-end");
        let end_node: Node = end_marker.into();

        if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
            return;
        }

        create_effect(move || {
            // 在产生副作用时捕获 Panic，防止整个应用崩溃，并允许 ErrorBoundary 捕获
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let view = self();

                // A. 清理旧节点 (Range Clean)
                // 删除 start_node 和 end_node 之间的所有节点
                // 这比追踪 mounted_nodes 更健壮，特别是对于嵌套的动态 View 或 Fragment 逃逸情况
                if let Some(parent) = start_node.parent_node() {
                    while let Some(sibling) = start_node.next_sibling() {
                        // 引用比较，到达结束锚点停止
                        if sibling == end_node {
                            break;
                        }
                        // 移除中间节点
                        let _ = parent.remove_child(&sibling);
                    }
                }

                // B. 准备新内容 (使用 DocumentFragment 收集节点)
                let fragment = document.create_document_fragment();
                let fragment_node: Node = fragment.clone().into();

                // 挂载到 Fragment
                view.mount(&fragment_node);

                // C. 插入到 DOM (在 end_marker 之前)
                if let Some(parent) = end_node.parent_node() {
                    let _ = parent.insert_before(&fragment_node, Some(&end_node));
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

                silex_core::error::handle_error(SilexError::Javascript(msg));
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
        let document = crate::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
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

impl<V: View, const N: usize> View for [V; N] {
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
            Err(e) => silex_core::error::handle_error(e),
        }
    }
}

// --- AnyView (Type Erasure) ---

/// 辅助特征，用于支持 Box<dyn View> 的移动语义挂载
pub trait Render {
    fn mount_boxed(self: Box<Self>, parent: &Node);
    fn clone_boxed(&self) -> Box<dyn Render>;
}

impl<V: View + Clone + 'static> Render for V {
    fn mount_boxed(self: Box<Self>, parent: &Node) {
        (*self).mount(parent)
    }
    fn clone_boxed(&self) -> Box<dyn Render> {
        Box::new(self.clone())
    }
}

/// 类型擦除的 View，可以持有任何 View 的实现。
/// 用于从同一个函数返回不同类型的 View（例如：主题）。
pub struct AnyView(Box<dyn Render>);

impl AnyView {
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        Self(Box::new(view))
    }
}

impl View for AnyView {
    fn mount(self, parent: &Node) {
        self.0.mount_boxed(parent)
    }
}

impl Clone for AnyView {
    fn clone(&self) -> Self {
        Self(self.0.clone_boxed())
    }
}

impl PartialEq for AnyView {
    fn eq(&self, _other: &Self) -> bool {
        // 对于类型擦除的 View，默认假设它们总是不同的，
        // 这样可以确保响应式系统总是重新渲染它们。
        // 这对于 Dynamic 组件来说是合理的行为。
        false
    }
}

pub trait IntoAnyView {
    fn into_any(self) -> AnyView;
}

impl<V: View + Clone + 'static> IntoAnyView for V {
    fn into_any(self) -> AnyView {
        AnyView::new(self)
    }
}

// --- Children & Fragment ---

/// 标准子组件类型，即类型擦除的 View
/// 允许组件存储子元素而无需泛型
pub type Children = AnyView;

impl Default for AnyView {
    fn default() -> Self {
        AnyView::new(())
    }
}

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnyView")
    }
}

/// 片段，用于容纳多个不同类型的子组件
#[derive(Default, Clone)]
pub struct Fragment(Vec<AnyView>);

impl Fragment {
    pub fn new(children: Vec<AnyView>) -> Self {
        Self(children)
    }
}

impl View for Fragment {
    fn mount(self, parent: &Node) {
        for child in self.0 {
            child.mount(parent);
        }
    }
}

// --- From Implementations for AnyView (for Builder Pattern / Into Support) ---

impl From<Element> for AnyView {
    fn from(v: Element) -> Self {
        AnyView::new(v)
    }
}

impl From<String> for AnyView {
    fn from(v: String) -> Self {
        AnyView::new(v)
    }
}

impl From<&str> for AnyView {
    fn from(v: &str) -> Self {
        AnyView::new(v.to_string())
    }
}

impl From<()> for AnyView {
    fn from(_: ()) -> Self {
        AnyView::new(())
    }
}

macro_rules! impl_from_primitive_for_anyview {
    ($($t:ty),*) => {
        $(
            impl From<$t> for AnyView {
                fn from(v: $t) -> Self {
                    AnyView::new(v)
                }
            }
        )*
    };
}
impl_from_primitive_for_anyview!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl<V: View + Clone + 'static> From<Vec<V>> for AnyView {
    fn from(v: Vec<V>) -> Self {
        AnyView::new(v)
    }
}

impl<V: View + Clone + 'static> From<Option<V>> for AnyView {
    fn from(v: Option<V>) -> Self {
        AnyView::new(v)
    }
}

// Manually impl From for Tuples since we can't do generic V due to conflict with self-impl
macro_rules! impl_from_tuple_for_anyview {
    ($($name:ident),*) => {
        impl<$($name: View + Clone + 'static),*> From<($($name,)*)> for AnyView {
            fn from(v: ($($name,)*)) -> Self {
                AnyView::new(v)
            }
        }
    }
}

impl_from_tuple_for_anyview!(A);
impl_from_tuple_for_anyview!(A, B);
impl_from_tuple_for_anyview!(A, B, C);
impl_from_tuple_for_anyview!(A, B, C, D);
impl_from_tuple_for_anyview!(A, B, C, D, E);

/// 一个辅助宏，用于简化从 `match` 表达式返回 `AnyView` 的操作。
///
/// 它会自动对每个分支的结果调用 `.into_any()`，从而允许不同类型的 View 在同一个 `match` 块中返回。
///
/// # 示例
///
/// ```rust, ignore
/// view_match!(route, {
///     AppRoute::Home => HomePage::new(),
///     AppRoute::Basics => "Basics Page",
///     AppRoute::NotFound => (),
/// })
/// ```
#[macro_export]
macro_rules! view_match {
    // 匹配分支中可以包含 guard (if condition)
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $crate::view::IntoAnyView::into_any($val),
            )*
        }
    };
}
