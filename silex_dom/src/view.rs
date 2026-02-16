use crate::attribute::PendingAttribute;
use crate::element::Element;
use silex_core::error::handle_error;
use silex_core::reactivity::{Derived, Effect, Memo, ReactiveBinary, ReadSignal, RwSignal, Signal};
use silex_core::traits::{Get, Track, WithUntracked};
use silex_core::{SilexError, SilexResult};
use std::fmt::Display;
use std::panic::{AssertUnwindSafe, catch_unwind};
use web_sys::Node;

/// 视图特征 (View Trait)
/// 核心特征：定义了如何将一个东西挂载到 DOM 上。
pub trait View {
    fn mount(self, parent: &Node);

    /// Apply forwarded attributes to this view.
    /// Default implementation does nothing (for Text, Fragment, etc.).
    /// Elements override this to actually apply attributes.
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}

    /// Convert this view into an AnyView (Type Erasure).
    ///
    /// By default, this wraps the view in a `Box<dyn Render>` (AnyView::Boxed).
    /// Specific types (Element, String, Fragment) override this to return
    /// their optimized Enum variant (AnyView::Element, AnyView::Text, etc.),
    /// avoiding heap allocation.
    fn into_any(self) -> AnyView
    where
        Self: Sized + Clone + 'static,
    {
        AnyView::Boxed(Box::new(self))
    }
}

// --- View Trait Implementations ---

// 1. 静态文本 (String, &str)
impl View for String {
    fn mount(self, parent: &Node) {
        let document = crate::document();
        let node = document.create_text_node(&self);
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
        }
    }

    fn into_any(self) -> AnyView {
        AnyView::Text(self)
    }
}

impl View for &str {
    fn mount(self, parent: &Node) {
        let document = crate::document();
        let node = document.create_text_node(self);
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
        }
    }

    fn into_any(self) -> AnyView {
        AnyView::Text(self.to_string())
    }
}

// 2. 基础类型支持
macro_rules! impl_view_for_primitive {
    ($($t:ty),*) => {
        $(
            impl View for $t {
                fn mount(self, parent: &Node) {
                    let document = crate::document();
                    let node = document.create_text_node(&self.to_string());
                    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
                        handle_error(e);
                    }
                }

                fn into_any(self) -> AnyView {
                    AnyView::Text(self.to_string())
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

    fn into_any(self) -> AnyView {
        AnyView::Empty
    }
}

// 3. 动态闭包支持 (Lazy View / Dynamic Text)
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
            handle_error(e);
            return;
        }

        let end_marker = document.create_comment("dyn-end");
        let end_node: Node = end_marker.into();

        if let Err(e) = parent.append_child(&end_node).map_err(SilexError::from) {
            handle_error(e);
            return;
        }

        Effect::new(move |_| {
            // 在产生副作用时捕获 Panic，防止整个应用崩溃，并允许 ErrorBoundary 捕获
            let result = catch_unwind(AssertUnwindSafe(|| {
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

                handle_error(SilexError::Javascript(msg));
            }
        });
    }
}

// 4. 直接 Signal 支持
impl<T> View for ReadSignal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
            return;
        }

        // 2. 创建副作用
        let signal = self;
        Effect::new(move |_| {
            let value = signal.get();
            node.set_node_value(Some(&value.to_string()));
        });
    }
}

impl<T> View for Memo<T>
where
    T: Display + Clone + PartialEq + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
            return;
        }

        // 2. 创建副作用
        let signal = self;
        Effect::new(move |_| {
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

impl<T> View for Signal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
            return;
        }

        // 2. 创建副作用
        let signal = self;
        Effect::new(move |_| {
            let value = signal.get();
            node.set_node_value(Some(&value.to_string()));
        });
    }
}

impl<S, F, U> View for Derived<S, F>
where
    S: WithUntracked + Track + Clone + 'static,
    F: Fn(&S::Value) -> U + Clone + 'static,
    U: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
            return;
        }

        // 2. 创建副作用
        let signal = self;
        Effect::new(move |_| {
            let value = signal.get();
            node.set_node_value(Some(&value.to_string()));
        });
    }
}

impl<L, R, F, U> View for ReactiveBinary<L, R, F>
where
    L: WithUntracked + Track + Clone + 'static,
    R: WithUntracked + Track + Clone + 'static,
    F: Fn(&L::Value, &R::Value) -> U + Clone + 'static,
    U: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        let document = crate::document();
        // 1. 创建占位符
        let node = document.create_text_node("");
        if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
            handle_error(e);
            return;
        }

        // 2. 创建副作用
        let signal = self;
        Effect::new(move |_| {
            let value = signal.get();
            node.set_node_value(Some(&value.to_string()));
        });
    }
}

// 5. 容器类型支持
impl<V: View> View for Option<V> {
    fn mount(self, parent: &Node) {
        if let Some(v) = self {
            v.mount(parent);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        if let Some(v) = self {
            v.apply_attributes(attrs);
        }
    }
}

impl<V: View> View for Vec<V> {
    fn mount(self, parent: &Node) {
        for v in self {
            v.mount(parent);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for v in self {
            v.apply_attributes(attrs.clone());
        }
    }
}

impl<V: View, const N: usize> View for [V; N] {
    fn mount(self, parent: &Node) {
        for v in self {
            v.mount(parent);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for v in self {
            v.apply_attributes(attrs.clone());
        }
    }
}

// 6. 元组支持
macro_rules! impl_view_for_tuple {
    ($($name:ident),*) => {
        impl<$($name: View),*> View for ($($name,)*) {
            #[allow(non_snake_case)]
            fn mount(self, parent: &Node) {
                let ($($name,)*) = self;
                $($name.mount(parent);)*
            }

            #[allow(non_snake_case)]
            fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
                let ($($name,)*) = self;
                $($name.apply_attributes(attrs.clone());)*
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

// 7. Result 支持
impl<V: View> View for SilexResult<V> {
    fn mount(self, parent: &Node) {
        match self {
            Ok(v) => v.mount(parent),
            Err(e) => handle_error(e),
        }
    }
}

// --- AnyView (Enum Optimization) ---

/// 辅助特征，用于支持 Box<dyn View> 的移动语义挂载
pub trait Render {
    fn mount_boxed(self: Box<Self>, parent: &Node);
    fn clone_boxed(&self) -> Box<dyn Render>;
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>);
}

impl<V: View + Clone + 'static> Render for V {
    fn mount_boxed(self: Box<Self>, parent: &Node) {
        (*self).mount(parent)
    }
    fn clone_boxed(&self) -> Box<dyn Render> {
        Box::new(self.clone())
    }
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>) {
        self.apply_attributes(attrs);
    }
}

/// 优化的 AnyView，使用 Enum 分发常见类型，减少 Box 开销。
pub enum AnyView {
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<AnyView>),
    Boxed(Box<dyn Render>),
}

impl AnyView {
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        view.into_any()
    }
}

impl View for AnyView {
    fn mount(self, parent: &Node) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount(parent),
            AnyView::Element(el) => el.mount(parent),
            AnyView::List(list) => {
                for child in list {
                    child.mount(parent);
                }
            }
            AnyView::Boxed(b) => b.mount_boxed(parent),
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}   // Cannot apply attributes to empty
            AnyView::Text(_) => {} // Typically cannot apply to text, unless wrapped (but Text impl View ignores it)
            AnyView::Element(el) => el.apply_attributes(attrs),
            AnyView::List(list) => {
                // Forwarding strategy: First Match
                for child in list {
                    child.apply_attributes(attrs.clone());
                }
            }
            AnyView::Boxed(b) => b.apply_attributes_boxed(attrs),
        }
    }
}

impl Clone for AnyView {
    fn clone(&self) -> Self {
        match self {
            AnyView::Empty => AnyView::Empty,
            AnyView::Text(s) => AnyView::Text(s.clone()),
            AnyView::Element(el) => AnyView::Element(el.clone()),
            AnyView::List(list) => AnyView::List(list.clone()),
            AnyView::Boxed(b) => AnyView::Boxed(b.clone_boxed()),
        }
    }
}

impl PartialEq for AnyView {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AnyView::Empty, AnyView::Empty) => true,
            (AnyView::Text(a), AnyView::Text(b)) => a == b,
            (AnyView::Element(a), AnyView::Element(b)) => a == b,
            (AnyView::List(a), AnyView::List(b)) => a == b,

            // Boxed is hard to compare partially.
            _ => false,
        }
    }
}

// --- Children & Fragment ---

/// 标准子组件类型，即类型擦除的 View
/// 允许组件存储子元素而无需泛型
pub type Children = AnyView;

impl Default for AnyView {
    fn default() -> Self {
        AnyView::Empty
    }
}

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "AnyView(Empty)"),
            Self::Text(arg0) => f.debug_tuple("AnyView(Text)").field(arg0).finish(),
            Self::Element(_) => write!(f, "AnyView(Element)"),
            Self::List(l) => f.debug_tuple("AnyView(List)").field(&l.len()).finish(),
            Self::Boxed(_) => write!(f, "AnyView(Boxed)"),
        }
    }
}

/// 片段，用于容纳多个不同类型的子组件
#[derive(Default, Clone)]
pub struct Fragment(pub Vec<AnyView>);

impl Fragment {
    pub fn new(children: Vec<AnyView>) -> Self {
        Self(children)
    }
}

impl View for Fragment {
    fn mount(self, parent: &Node) {
        self.0.mount(parent);
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.0.apply_attributes(attrs);
    }

    fn into_any(self) -> AnyView {
        AnyView::List(self.0)
    }
}

// --- From Implementations for AnyView (for Builder Pattern / Into Support) ---

impl From<Element> for AnyView {
    fn from(v: Element) -> Self {
        AnyView::Element(v)
    }
}

impl From<String> for AnyView {
    fn from(v: String) -> Self {
        AnyView::Text(v)
    }
}

impl From<&str> for AnyView {
    fn from(v: &str) -> Self {
        AnyView::Text(v.to_string())
    }
}

impl From<()> for AnyView {
    fn from(_: ()) -> Self {
        AnyView::Empty
    }
}

macro_rules! impl_from_primitive_for_anyview {
    ($($t:ty),*) => {
        $(
            impl From<$t> for AnyView {
                fn from(v: $t) -> Self {
                    AnyView::Text(v.to_string())
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
        match v {
            Some(val) => AnyView::new(val),
            None => AnyView::Empty,
        }
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
                $pat $(if $guard)? => $crate::view::View::into_any($val),
            )*
        }
    };
}
