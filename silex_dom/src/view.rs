use crate::attribute::PendingAttribute;
use crate::element::Element;
use silex_core::error::handle_error;
use silex_core::reactivity::{
    Constant, DerivedPayload, Effect, Memo, OpPayload, ReadSignal, RwSignal, Signal, SignalSlice,
};
use silex_core::traits::{RxBase, RxGet, RxInternal};
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

    /// Convert this view into an AnyView (Type Erasure without Clone requirement).
    fn into_any(self) -> AnyView
    where
        Self: Sized + 'static,
    {
        AnyView::Unique(Box::new(self))
    }

    /// Convert this view into a SharedView (Type Erasure with Clone requirement).
    fn into_shared(self) -> SharedView
    where
        Self: Sized + Clone + 'static,
    {
        SharedView::SharedBoxed(Box::new(self))
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

// --- View Trait Implementations ---

// 1. 静态文本 (String, &str)
impl View for String {
    fn mount(self, parent: &Node) {
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
    fn mount(self, parent: &Node) {
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
                fn mount(self, parent: &Node) {
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
    fn mount(self, _parent: &Node) {}

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

// 3.5 Rx Delegation (Delegates to inner payload if it implements View)
impl<F, M> View for silex_core::Rx<F, M>
where
    F: View,
{
    fn mount(self, parent: &Node) {
        self.0.mount(parent);
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.0.apply_attributes(attrs);
    }
}

// 3.6 Type closure delegation
impl<V> View for std::rc::Rc<dyn Fn() -> V>
where
    V: View + 'static,
{
    fn mount(self, parent: &Node) {
        let f = self.clone();
        (move || f()).mount(parent);
    }
}

// --- 响应式文本归一化内核 (Reactive Text Consolidation Kernel) ---

trait ReactiveText: 'static {
    fn track(&self);
    fn render(&self) -> String;
}

/// 非泛型内核函数：负责处理所有响应式文本更新。
/// 通过类型擦除避免为每种计算生成独立的 Effect 代码，大幅减小二进制体积。
fn mount_reactive_text_(parent: &Node, rx: Box<dyn ReactiveText>) {
    let document = crate::document();
    let node = document.create_text_node("");
    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
        handle_error(e);
        return;
    }

    Effect::new(move |_| {
        rx.track();
        let value = rx.render();
        node.set_node_value(Some(&value));
    });
}

// 为各种类型实现 ReactiveText 桥接

impl<T> ReactiveText for ReadSignal<T>
where
    T: Display + Clone + 'static,
{
    fn track(&self) {
        RxBase::track(self);
    }
    fn render(&self) -> String {
        self.get_untracked().to_string()
    }
}

impl<T> ReactiveText for Memo<T>
where
    T: Display + Clone + PartialEq + 'static,
{
    fn track(&self) {
        RxBase::track(self);
    }
    fn render(&self) -> String {
        self.get_untracked().to_string()
    }
}

impl<T> ReactiveText for Signal<T>
where
    T: Display + Clone + 'static,
{
    fn track(&self) {
        RxBase::track(self);
    }
    fn render(&self) -> String {
        self.get_untracked().to_string()
    }
}

impl<S, F, U> ReactiveText for DerivedPayload<S, F>
where
    Self: RxInternal<Value = U> + Clone + 'static,
    U: Display + 'static,
{
    fn track(&self) {
        RxBase::track(self);
    }
    fn render(&self) -> String {
        self.rx_try_with_untracked(|v: &U| v.to_string())
            .unwrap_or_default()
    }
}

impl<U, const N: usize> ReactiveText for OpPayload<U, N>
where
    Self: RxInternal<Value = U> + Clone + 'static,
    U: Display + 'static,
{
    fn track(&self) {
        RxBase::track(self);
    }
    fn render(&self) -> String {
        self.rx_try_with_untracked(|v: &U| v.to_string())
            .unwrap_or_default()
    }
}

impl<S, F, O> ReactiveText for SignalSlice<S, F, O>
where
    Self: RxInternal<Value = O> + Clone + 'static,
    O: Display + ?Sized + 'static,
{
    fn track(&self) {
        RxBase::track(self);
    }
    fn render(&self) -> String {
        self.rx_try_with_untracked(|v: &O| v.to_string())
            .unwrap_or_default()
    }
}

// 4. 直接 Signal 支持 (重定向到归一化内核)
impl<T> View for ReadSignal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self));
    }
}

impl<T> View for Memo<T>
where
    T: Display + Clone + PartialEq + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self));
    }
}

impl<T> View for RwSignal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self.read_signal()));
    }
}

impl<T> View for Signal<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self));
    }
}

// 4. 通用 RxInternal 支持 (Display 类型)
// 4.1 DerivedPayload View (Text Update)
impl<S, F, U> View for DerivedPayload<S, F>
where
    Self: RxInternal<Value = U> + Clone + 'static,
    U: Display + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self));
    }
}

// 4.2 OpPayload View (Text Update)
impl<U, const N: usize> View for OpPayload<U, N>
where
    Self: RxInternal<Value = U> + Clone + 'static,
    U: Display + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self));
    }
}

// 4.4 SignalSlice View (Text Update)
impl<S, F, O> View for SignalSlice<S, F, O>
where
    Self: RxInternal<Value = O> + Clone + 'static,
    O: Display + ?Sized + 'static,
{
    fn mount(self, parent: &Node) {
        mount_reactive_text_(parent, Box::new(self));
    }
}

// 4.4 Constant View (Static Text)
impl<T> View for Constant<T>
where
    T: Display + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        mount_text_node(parent, &self.0.to_string());
    }

    fn into_any(self) -> AnyView {
        AnyView::Text(self.0.to_string())
    }

    fn into_shared(self) -> SharedView {
        SharedView::Text(self.0.to_string())
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

// --- AnyView & SharedView (Type Erasure & Enum Optimization) ---

/// 辅助特征（不要求 Clone，移动语义挂载）
pub trait RenderOnce {
    fn mount_boxed(self: Box<Self>, parent: &Node);
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>);
}

impl<V: View + 'static> RenderOnce for V {
    fn mount_boxed(self: Box<Self>, parent: &Node) {
        (*self).mount(parent)
    }
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>) {
        self.apply_attributes(attrs);
    }
}

/// 辅助特征（支持克隆）
pub trait RenderShared: RenderOnce {
    fn clone_boxed(&self) -> Box<dyn RenderShared>;
    fn into_once_boxed(self: Box<Self>) -> Box<dyn RenderOnce>;
}

impl<V: View + Clone + 'static> RenderShared for V {
    fn clone_boxed(&self) -> Box<dyn RenderShared> {
        Box::new(self.clone())
    }
    fn into_once_boxed(self: Box<Self>) -> Box<dyn RenderOnce> {
        self
    }
}

/// 优化的 SharedView，专用于需要重复使用或需要 Children 的组件边界
#[derive(Default)]
pub enum SharedView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<SharedView>),
    SharedBoxed(Box<dyn RenderShared>),
}

/// 优化的 AnyView，作为所有视图类型擦除的终点（不要求 Clone）
#[derive(Default)]
pub enum AnyView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<AnyView>),
    Unique(Box<dyn RenderOnce>),
    FromShared(SharedView),
}

impl SharedView {
    pub fn new<V: View + Clone + 'static>(view: V) -> Self {
        view.into_shared()
    }
}

impl AnyView {
    pub fn new<V: View + 'static>(view: V) -> Self {
        view.into_any()
    }
}

impl View for SharedView {
    fn mount(self, parent: &Node) {
        match self {
            SharedView::Empty => {}
            SharedView::Text(s) => s.mount(parent),
            SharedView::Element(el) => el.mount(parent),
            SharedView::List(list) => {
                for child in list {
                    child.mount(parent);
                }
            }
            SharedView::SharedBoxed(b) => b.mount_boxed(parent),
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        match self {
            SharedView::Empty => {}
            SharedView::Text(_) => {}
            SharedView::Element(el) => el.apply_attributes(attrs),
            SharedView::List(list) => {
                for child in list {
                    child.apply_attributes(attrs.clone());
                }
            }
            SharedView::SharedBoxed(b) => b.apply_attributes_boxed(attrs),
        }
    }

    fn into_any(self) -> AnyView {
        AnyView::FromShared(self)
    }

    fn into_shared(self) -> SharedView {
        self
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
            AnyView::Unique(b) => b.mount_boxed(parent),
            AnyView::FromShared(s) => s.mount(parent),
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(_) => {}
            AnyView::Element(el) => el.apply_attributes(attrs),
            AnyView::List(list) => {
                for child in list {
                    child.apply_attributes(attrs.clone());
                }
            }
            AnyView::Unique(b) => b.apply_attributes_boxed(attrs),
            AnyView::FromShared(s) => s.apply_attributes(attrs),
        }
    }

    fn into_any(self) -> AnyView {
        self
    }
}

impl Clone for SharedView {
    fn clone(&self) -> Self {
        match self {
            SharedView::Empty => SharedView::Empty,
            SharedView::Text(s) => SharedView::Text(s.clone()),
            SharedView::Element(el) => SharedView::Element(el.clone()),
            SharedView::List(list) => SharedView::List(list.clone()),
            SharedView::SharedBoxed(b) => SharedView::SharedBoxed(b.clone_boxed()),
        }
    }
}

impl PartialEq for SharedView {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SharedView::Empty, SharedView::Empty) => true,
            (SharedView::Text(a), SharedView::Text(b)) => a == b,
            (SharedView::Element(a), SharedView::Element(b)) => a == b,
            (SharedView::List(a), SharedView::List(b)) => a == b,
            _ => false,
        }
    }
}

// --- Children & Fragment ---

/// 标准子组件类型，即受 Clone 保护的擦除 SharedView
pub type Children = SharedView;

impl std::fmt::Debug for AnyView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "AnyView(Empty)"),
            Self::Text(arg0) => f.debug_tuple("AnyView(Text)").field(arg0).finish(),
            Self::Element(_) => write!(f, "AnyView(Element)"),
            Self::List(l) => f.debug_tuple("AnyView(List)").field(&l.len()).finish(),
            Self::Unique(_) => write!(f, "AnyView(Unique)"),
            Self::FromShared(s) => f.debug_tuple("AnyView(FromShared)").field(s).finish(),
        }
    }
}

impl std::fmt::Debug for SharedView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "SharedView(Empty)"),
            Self::Text(arg0) => f.debug_tuple("SharedView(Text)").field(arg0).finish(),
            Self::Element(_) => write!(f, "SharedView(Element)"),
            Self::List(l) => f.debug_tuple("SharedView(List)").field(&l.len()).finish(),
            Self::SharedBoxed(_) => write!(f, "SharedView(SharedBoxed)"),
        }
    }
}

/// 片段，用于容纳多个不同类型的子组件
#[derive(Default, Clone)]
pub struct Fragment(pub Vec<SharedView>);

impl Fragment {
    pub fn new(children: Vec<SharedView>) -> Self {
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
        AnyView::FromShared(SharedView::List(self.0))
    }

    fn into_shared(self) -> SharedView {
        SharedView::List(self.0)
    }
}

// --- From Implementations for Type Erasure ---

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

impl From<Element> for SharedView {
    fn from(v: Element) -> Self {
        SharedView::Element(v)
    }
}
impl From<String> for SharedView {
    fn from(v: String) -> Self {
        SharedView::Text(v)
    }
}
impl From<&str> for SharedView {
    fn from(v: &str) -> Self {
        SharedView::Text(v.to_string())
    }
}
impl From<()> for SharedView {
    fn from(_: ()) -> Self {
        SharedView::Empty
    }
}

macro_rules! impl_from_primitive {
    ($($t:ty),*) => {
        $(
            impl From<$t> for AnyView {
                fn from(v: $t) -> Self {
                    AnyView::Text(v.to_string())
                }
            }

            impl From<$t> for SharedView {
                fn from(v: $t) -> Self {
                    SharedView::Text(v.to_string())
                }
            }
        )*
    };
}
impl_from_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, isize, usize, f32, f64, bool, char
);

impl<V: View + 'static> From<Vec<V>> for AnyView {
    fn from(v: Vec<V>) -> Self {
        AnyView::List(v.into_iter().map(|item| item.into_any()).collect())
    }
}
impl<V: View + Clone + 'static> From<Vec<V>> for SharedView {
    fn from(v: Vec<V>) -> Self {
        SharedView::List(v.into_iter().map(|item| item.into_shared()).collect())
    }
}

impl<V: View + 'static> From<Option<V>> for AnyView {
    fn from(v: Option<V>) -> Self {
        match v {
            Some(val) => AnyView::new(val),
            None => AnyView::Empty,
        }
    }
}
impl<V: View + Clone + 'static> From<Option<V>> for SharedView {
    fn from(v: Option<V>) -> Self {
        match v {
            Some(val) => SharedView::new(val),
            None => SharedView::Empty,
        }
    }
}

macro_rules! impl_from_tuple {
    ($($name:ident),*) => {
        impl<$($name: View + 'static),*> From<($($name,)*)> for AnyView {
            fn from(v: ($($name,)*)) -> Self {
                AnyView::new(v)
            }
        }

        impl<$($name: View + Clone + 'static),*> From<($($name,)*)> for SharedView {
            fn from(v: ($($name,)*)) -> Self {
                SharedView::new(v)
            }
        }
    }
}

impl_from_tuple!(A);
impl_from_tuple!(A, B);
impl_from_tuple!(A, B, C);
impl_from_tuple!(A, B, C, D);
impl_from_tuple!(A, B, C, D, E);

/// 一个辅助宏，用于简化从 `match` 表达式返回 `SharedView` 的操作。
///
/// 它会自动对每个分支的结果调用 `.into_shared()`，从而允许不同类型的 View 在同一个 `match` 块中返回。
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
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $crate::view::View::into_shared($val),
            )*
        }
    };
}

#[macro_export]
macro_rules! any_view_match {
    ($target:expr, { $($pat:pat $(if $guard:expr)? => $val:expr),* $(,)? }) => {
        match $target {
            $(
                $pat $(if $guard)? => $crate::view::View::into_any($val),
            )*
        }
    };
}
