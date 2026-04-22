pub mod any;
pub mod reactive;

pub use any::*;
pub use reactive::*;

use crate::attribute::PendingAttribute;
use silex_core::error::handle_error;
use silex_core::logic::Map;
use silex_core::reactivity::{Effect, NodeId, create_scope, dispose};
use silex_core::traits::{IntoRx, IntoSignal, RxValue};
use silex_core::{Rx, RxValueKind, SilexError, SilexResult};
use std::cell::RefCell;
use std::ops::Deref;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use web_sys::Node;

/// 递归视图链辅助结构 - 空节点
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewNil;

/// 递归视图链辅助结构 - 构造节点
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewCons<H, T>(pub H, pub T);

/// 属性应用特征 (ApplyAttributes Trait)
pub trait ApplyAttributes {
    /// Apply forwarded attributes to this view.
    /// Default implementation does nothing (for Text, Fragment, etc.).
    /// Elements override this to actually apply attributes.
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}
}

/// 挂载特征 - 消耗型 (Mount Trait)
pub trait Mount {
    /// Mount this view to a parent node with a set of pending attributes.
    /// This is the primary entry point for mounting views.
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>);
}

/// 挂载特征 - 引用型 (MountRef Trait)
pub trait MountRef {
    /// Optimized mounting from a reference to avoid redundant clones.
    /// For types that are cheap to clone (like Rc-based Elements), this can be just a clone + mount.
    /// For expensive types (like Strings or Fragments), this should be implemented without cloning.
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>);
}

/// 组件宏内部使用的属性包装器。
///
/// 该类型用于统一组件在 `mount` (Owned) 和 `mount_ref` (Borrowed) 路径下的行为。
/// 它不实现 `Copy` 特征（即使 T 是 Copy），其 `Clone` 实现始终会产生一个 `Owned` 变体，
/// 从而允许在闭包中安全地进行 `'static` 捕获，同时消除 Clippy 对 Copy 类型冗余克隆的警告。
pub enum Prop<'a, T> {
    Owned(T),
    Borrowed(&'a T),
}

impl<'a, T> Prop<'a, T> {
    pub fn new_borrowed(value: &'a T) -> Self {
        Self::Borrowed(value)
    }

    pub fn new_owned(value: T) -> Self {
        Self::Owned(value)
    }

    /// 兼容性方法
    pub fn new(value: &'a T) -> Self {
        Self::Borrowed(value)
    }
}

impl<'a, T: Clone> Prop<'a, T> {
    /// 注意：Prop 的 .clone() 返回的是 T 而不是 Prop！
    ///
    /// 这里的 inherent method 优先级高于下方的 Clone trait 实现。
    /// 这是为了让组件代码通过 .clone() 拿到的总是 Owned 类型，
    /// 从而可以安全地 move 进入 'static 闭包，且不触发 Clippy 对 Copy 类型冗余克隆的警告。
    #[allow(clippy::should_implement_trait)]
    pub fn clone(&self) -> T {
        match self {
            Self::Owned(v) => v.clone(),
            Self::Borrowed(v) => (*v).clone(),
        }
    }

    /// Consume the wrapper and return an owned value.
    /// Borrowed values are cloned on demand.
    pub fn into_owned(self) -> T {
        match self {
            Self::Owned(v) => v,
            Self::Borrowed(v) => (*v).clone(),
        }
    }
}

impl<'a, T: Clone> Clone for Prop<'a, T> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(v) => Self::Owned(v.clone()),
            Self::Borrowed(v) => Self::Owned((*v).clone()),
        }
    }
}

impl<'a, T: Copy> Copy for Prop<'a, T> {}

/// 属性转换特征，允许组件构建器接受 Prop<T> 或 T。
pub trait PropInto<T> {
    fn prop_into(self) -> T;
}

impl<T> PropInto<T> for T {
    #[inline(always)]
    fn prop_into(self) -> T {
        self
    }
}

impl<'a, T: Clone> PropInto<T> for Prop<'a, T> {
    #[inline(always)]
    fn prop_into(self) -> T {
        self.clone()
    }
}

impl<'a, T: MountRef> MountRef for Prop<'a, T> {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            Self::Owned(v) => v.mount_ref(parent, attrs),
            Self::Borrowed(v) => v.mount_ref(parent, attrs),
        }
    }
}

impl<'a, T> ApplyAttributes for Prop<'a, T> {}

impl<'a, T> Mount for Prop<'a, T>
where
    T: MountRef,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            Self::Owned(v) => v.mount_ref(parent, attrs),
            Self::Borrowed(v) => v.mount_ref(parent, attrs),
        }
    }
}

impl<'a, T> Deref for Prop<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(v) => v,
            Self::Borrowed(v) => v,
        }
    }
}

impl<'a, T: RxValue> RxValue for Prop<'a, T> {
    type Value = T::Value;
}

impl<'a, T> IntoRx for Prop<'a, T>
where
    T: IntoRx + Clone,
{
    type RxType = T::RxType;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        self.clone().into_rx()
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.deref().is_constant()
    }
}

impl<'a, T> IntoSignal for Prop<'a, T>
where
    T: IntoSignal + Clone,
{
    #[inline(always)]
    fn into_signal(self) -> silex_core::reactivity::Signal<Self::Value>
    where
        Self: Sized + silex_core::traits::RxData,
        Self::Value: Sized + silex_core::traits::RxCloneData,
    {
        self.clone().into_signal()
    }
}

impl<'a, T> Prop<'a, T>
where
    T: Clone + Map + 'static,
    T::Value: Sized + 'static,
{
    pub fn map<U>(&self, f: impl Fn(&T::Value) -> U + 'static) -> Rx<U, RxValueKind>
    where
        U: 'static,
    {
        match self {
            Self::Owned(v) => v.clone().map(f),
            Self::Borrowed(v) => (*v).clone().map(f),
        }
    }

    pub fn map_fn<U>(&self, f: fn(&T::Value) -> U) -> Rx<U, RxValueKind>
    where
        U: 'static,
    {
        match self {
            Self::Owned(v) => v.clone().map_fn(f),
            Self::Borrowed(v) => (*v).clone().map_fn(f),
        }
    }
}

impl<'a, T> std::fmt::Debug for Prop<'a, T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owned(v) => v.fmt(f),
            Self::Borrowed(v) => v.fmt(f),
        }
    }
}

impl<'a, T> std::fmt::Display for Prop<'a, T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owned(v) => v.fmt(f),
            Self::Borrowed(v) => v.fmt(f),
        }
    }
}

macro_rules! impl_forward_binop_copy {
    ($trait:ident, $method:ident) => {
        impl<'a, T, Rhs> std::ops::$trait<Rhs> for Prop<'a, T>
        where
            T: Copy + std::ops::$trait<Rhs>,
        {
            type Output = <T as std::ops::$trait<Rhs>>::Output;

            fn $method(self, rhs: Rhs) -> Self::Output {
                self.deref().$method(rhs)
            }
        }
    };
}

impl_forward_binop_copy!(Add, add);
impl_forward_binop_copy!(Sub, sub);
impl_forward_binop_copy!(Mul, mul);
impl_forward_binop_copy!(Div, div);

/// 视图转换扩展 (Mount Extensions)
pub trait View: MountRef + Mount + ApplyAttributes + 'static {
    /// Convert this view into an AnyView.
    fn into_any(self) -> AnyView;
}

impl<T: MountRef + Mount + ApplyAttributes + 'static> View for T {
    fn into_any(self) -> AnyView {
        AnyView::new(self)
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
impl ApplyAttributes for String {}
impl Mount for String {
    fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, &self);
    }
}
impl MountRef for String {
    fn mount_ref(&self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, self);
    }
}

impl MountRef for str {
    fn mount_ref(&self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, self);
    }
}

impl MountRef for &str {
    fn mount_ref(&self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, self);
    }
}

impl ApplyAttributes for &str {}
impl Mount for &str {
    fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, self);
    }
}

// 1.1 Cow support
impl<'a> ApplyAttributes for std::borrow::Cow<'a, str> {}
impl<'a> Mount for std::borrow::Cow<'a, str> {
    fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, &self);
    }
}
impl<'a> MountRef for std::borrow::Cow<'a, str> {
    fn mount_ref(&self, parent: &Node, _attrs: Vec<PendingAttribute>) {
        mount_text_node(parent, self.as_ref());
    }
}
// 2. 基础类型支持
macro_rules! impl_mount_for_primitive {
    ($($t:ty),*) => {
        $(
            impl ApplyAttributes for $t {}
            impl Mount for $t {
                fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
                    mount_text_node(parent, &self.to_string());
                }
            }
            impl MountRef for $t {
                fn mount_ref(&self, parent: &Node, _attrs: Vec<PendingAttribute>) {
                    mount_text_node(parent, &self.to_string());
                }
            }
        )*
    };
}

impl_mount_for_primitive!(
    i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize, f32, f64, bool, char
);

impl ApplyAttributes for () {}
impl Mount for () {
    fn mount(self, _parent: &Node, _attrs: Vec<PendingAttribute>) {}
}
impl MountRef for () {
    fn mount_ref(&self, _parent: &Node, _attrs: Vec<PendingAttribute>) {}
}

// 3. 动态闭包支持 (Lazy View / Dynamic Text)
impl<F, V> ApplyAttributes for F
where
    F: Fn() -> V + Clone + 'static,
    V: Mount + 'static,
{
}

impl<F, V> Mount for F
where
    F: Fn() -> V + Clone + 'static,
    V: Mount + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_dynamic_view_universal(
            parent,
            attrs,
            RenderThunk::new(move |args| {
                let (p, a) = args;
                self().mount(&p, a);
            }),
        );
    }
}

impl<F, V> MountRef for F
where
    F: Fn() -> V + Clone + 'static,
    V: Mount + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let this = self.clone();
        mount_dynamic_view_universal(
            parent,
            attrs,
            RenderThunk::new(move |args| {
                let (p, a) = args;
                this().mount(&p, a);
            }),
        );
    }
}

/// 非泛型的动态视图挂载内核，作为所有响应式/延迟视图的调度终点，用于减少单态化膨胀。
pub fn mount_dynamic_view_universal(
    parent: &Node,
    attrs: Vec<PendingAttribute>,
    renderer: RenderThunk,
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

    Effect::new(move |_| {
        let start_node = start_node.clone();
        let end_node = end_node.clone();
        let document = document.clone();
        let attrs = attrs.clone();
        let renderer = &renderer;

        let result = catch_unwind(AssertUnwindSafe(move || {
            // 1. 在生产新视图前，先同步清理旧 DOM 节点。
            if let Some(parent) = start_node.parent_node() {
                while let Some(sibling) = start_node.next_sibling() {
                    if sibling == end_node {
                        break;
                    }
                    let _ = parent.remove_child(&sibling);
                }
            }

            // 2. 使用 DocumentFragment 进行物理隔离增强，确保挂载位置精确性
            let fragment = document.create_document_fragment();
            let fragment_node: Node = fragment.clone().into();

            // 在当前 Effect 环境下执行渲染，确保护留所有信号追踪
            renderer.call((fragment_node.clone(), attrs));

            // 3. 将生产的内容插入锚点之间
            if let Some(parent) = end_node.parent_node() {
                let _ = parent.insert_before(&fragment_node, Some(&end_node));
            }
        }));

        if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                format!("Panic in Dynamic View: {}", s)
            } else if let Some(s) = payload.downcast_ref::<String>() {
                format!("Panic in Dynamic View: {}", s)
            } else {
                "Unknown Panic in Dynamic View".to_string()
            };

            handle_error(SilexError::Javascript(msg));
        }
    });
}

fn clear_nodes_between(start_node: &Node, end_node: &Node) {
    if let Some(parent) = start_node.parent_node() {
        while let Some(sibling) = start_node.next_sibling() {
            if sibling == *end_node {
                break;
            }
            let _ = parent.remove_child(&sibling);
        }
    }
}

/// 带分支缓存的动态视图挂载内核。
///
/// 当 key 未变化时，当前分支会保持原样，避免重复清理和重建。
pub fn mount_dynamic_view_cached<K, KeyFn, RenderFn>(
    parent: &Node,
    attrs: Vec<PendingAttribute>,
    key_fn: KeyFn,
    renderer: RenderFn,
) where
    K: PartialEq + Clone + 'static,
    KeyFn: Fn() -> K + Clone + 'static,
    RenderFn: Fn(K, (Node, Vec<PendingAttribute>)) + 'static,
{
    let document = crate::document();

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

    let active_state = Rc::new(RefCell::new(None::<(K, NodeId)>));

    Effect::new(move |_| {
        let start_node = start_node.clone();
        let end_node = end_node.clone();
        let document = document.clone();
        let attrs = attrs.clone();
        let renderer = &renderer;
        let active_state = active_state.clone();
        let key_fn = key_fn.clone();

        let result = catch_unwind(AssertUnwindSafe(move || {
            let next_key = key_fn();

            let unchanged = active_state
                .borrow()
                .as_ref()
                .is_some_and(|(current_key, _)| current_key == &next_key);

            if unchanged {
                return;
            }

            if let Some((_, scope_id)) = active_state.borrow_mut().take() {
                clear_nodes_between(&start_node, &end_node);
                dispose(scope_id);
            } else {
                clear_nodes_between(&start_node, &end_node);
            }

            let fragment = document.create_document_fragment();
            let fragment_node: Node = fragment.clone().into();
            let fragment_node_for_scope = fragment_node.clone();
            let attrs_for_scope = attrs.clone();
            let next_key_for_render = next_key.clone();

            let scope_id = create_scope(move || {
                renderer(
                    next_key_for_render.clone(),
                    (fragment_node_for_scope.clone(), attrs_for_scope.clone()),
                );
            });

            if let Some(parent) = end_node.parent_node() {
                let _ = parent.insert_before(&fragment_node, Some(&end_node));
            }

            *active_state.borrow_mut() = Some((next_key, scope_id));
        }));

        if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                format!("Panic in Cached Dynamic View: {}", s)
            } else if let Some(s) = payload.downcast_ref::<String>() {
                format!("Panic in Cached Dynamic View: {}", s)
            } else {
                "Unknown Panic in Cached Dynamic View".to_string()
            };

            handle_error(SilexError::Javascript(msg));
        }
    });
}

/// 根据分支 key 选择 `AnyView` 的缓存挂载辅助层。
pub fn mount_branch_cached<K, KeyFn, BranchFn>(
    parent: &Node,
    attrs: Vec<PendingAttribute>,
    key_fn: KeyFn,
    branch_fn: BranchFn,
) where
    K: PartialEq + Clone + 'static,
    KeyFn: Fn() -> K + Clone + 'static,
    BranchFn: Fn(K) -> AnyView + 'static,
{
    mount_dynamic_view_cached(parent, attrs, key_fn, move |key, (p, a)| {
        branch_fn(key).mount_ref(&p, a);
    });
}

// 3.6 Type closure delegation
impl<V> ApplyAttributes for std::rc::Rc<dyn Fn() -> V> where V: Mount + 'static {}

impl<V> Mount for std::rc::Rc<dyn Fn() -> V>
where
    V: Mount + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let f = self;
        (move || f()).mount(parent, attrs);
    }
}

impl<V> MountRef for std::rc::Rc<dyn Fn() -> V>
where
    V: Mount + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        let f = self.clone();
        (move || f()).mount(parent, attrs);
    }
}

// 5. 容器类型支持
impl<V: ApplyAttributes> ApplyAttributes for Option<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        if let Some(v) = self {
            v.apply_attributes(attrs);
        }
    }
}

impl<V: Mount> Mount for Option<V> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        if let Some(v) = self {
            v.mount(parent, attrs);
        }
    }
}

impl<V: MountRef> MountRef for Option<V> {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        if let Some(v) = self {
            v.mount_ref(parent, attrs.clone());
        }
    }
}

impl<V: ApplyAttributes> ApplyAttributes for Vec<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for v in self {
            v.apply_attributes(attrs.clone());
        }
    }
}

impl<V: Mount> Mount for Vec<V> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, v) in self.into_iter().enumerate() {
            v.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }
}

impl<V: MountRef> MountRef for Vec<V> {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, v) in self.iter().enumerate() {
            v.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }
}

impl<V: ApplyAttributes, const N: usize> ApplyAttributes for [V; N] {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for v in self {
            v.apply_attributes(attrs.clone());
        }
    }
}

impl<V: Mount, const N: usize> Mount for [V; N] {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, v) in self.into_iter().enumerate() {
            v.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }
}

impl<V: MountRef, const N: usize> MountRef for [V; N] {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, v) in self.iter().enumerate() {
            v.mount_ref(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }
}

// 6. 递归元组支持 (Recursive Tuple Support)

impl ApplyAttributes for ViewNil {}
impl Mount for ViewNil {
    fn mount(self, _parent: &Node, _attrs: Vec<PendingAttribute>) {}
}
impl MountRef for ViewNil {
    fn mount_ref(&self, _parent: &Node, _attrs: Vec<PendingAttribute>) {}
}

impl<H: ApplyAttributes, T: ApplyAttributes> ApplyAttributes for ViewCons<H, T> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.0.apply_attributes(attrs.clone());
        self.1.apply_attributes(attrs);
    }
}

impl<H: Mount, T: Mount> Mount for ViewCons<H, T> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        // 头节点接收 attributes
        self.0.mount(parent, attrs);
        // 后续链表不再接受 attributes (避免重复应用)
        self.1.mount(parent, Vec::new());
    }
}

impl<H: MountRef, T: MountRef> MountRef for ViewCons<H, T> {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        self.0.mount_ref(parent, attrs);
        self.1.mount_ref(parent, Vec::new());
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
impl<V: ApplyAttributes> ApplyAttributes for SilexResult<V> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        if let Ok(v) = self {
            v.apply_attributes(attrs)
        }
    }
}

impl<V: Mount> Mount for SilexResult<V> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            Ok(v) => v.mount(parent, attrs),
            Err(e) => handle_error(e),
        }
    }
}

impl<V: MountRef> MountRef for SilexResult<V> {
    fn mount_ref(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            Ok(v) => v.mount_ref(parent, attrs),
            Err(e) => handle_error(e.clone()),
        }
    }
}
