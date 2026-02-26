use crate::attribute::PendingAttribute;
use crate::element::Element;
use silex_core::error::handle_error;
use silex_core::reactivity::{Effect, Signal};
use silex_core::{SilexError, SilexResult};
use std::fmt::Display;
use std::panic::{AssertUnwindSafe, catch_unwind};
use web_sys::Node;

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

// --- View Trait Implementations ---

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

// --- 响应式文本归一化内核 (Reactive Text Consolidation Kernel) ---

/// 泛型内核函数：负责处理所有响应式文本更新。
/// 移除 Box<dyn ReactiveText>，通过直接接受 Signal<T> 避免昂贵的装箱和虚函数开销，提升运行时渲染性能。
pub(crate) fn mount_reactive_text<T: Display + Clone + 'static>(parent: &Node, rx: Signal<T>) {
    let node_id = rx.ensure_node_id();
    let converter = crate::attribute::primitive_to_string_erased::<T>;
    mount_erased_reactive_text_internal(parent, node_id, converter);
}

fn mount_erased_reactive_text_internal(
    parent: &Node,
    node_id: silex_core::reactivity::NodeId,
    converter: crate::attribute::ErasedStringConverter,
) {
    let document = crate::document();
    let node = document.create_text_node("");
    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
        handle_error(e);
        return;
    }

    Effect::new(move |_| {
        let value = converter(node_id);
        node.set_node_value(Some(&value));
    });
}

// --- 响应式组件视图内核 (Reactive View Core) ---

pub(crate) type ErasedViewConverter = fn(silex_core::reactivity::NodeId) -> SharedView;

pub(crate) fn mount_reactive_view<T: View + Clone + 'static>(
    parent: &Node,
    rx: Signal<T>,
    attrs: Vec<PendingAttribute>,
) {
    let node_id = rx.ensure_node_id();
    let converter = view_to_shared_erased::<T>;
    mount_erased_reactive_view_internal(parent, node_id, attrs, converter);
}

fn view_to_shared_erased<T: View + Clone + 'static>(
    node_id: silex_core::reactivity::NodeId,
) -> SharedView {
    use silex_core::traits::RxRead;
    let rx = silex_core::reactivity::Signal::<T>::Derived(node_id, std::marker::PhantomData);
    rx.with(|v| v.clone().into_shared())
}

fn mount_erased_reactive_view_internal(
    parent: &Node,
    node_id: silex_core::reactivity::NodeId,
    attrs: Vec<PendingAttribute>,
    converter: ErasedViewConverter,
) {
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
            let view = converter(node_id);

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

// 4. Rx wrapper support (Unified entry point for reactive normalization)
impl<V, M> View for silex_core::Rx<V, M>
where
    V: silex_core::traits::RxCloneData + Sized + 'static,
    M: 'static,
    silex_core::reactivity::Signal<V>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoSignal;
        self.into_signal().mount(parent, attrs);
    }

    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}
}

macro_rules! impl_view_for_reactive_erased_views {
    ($($ty:ty),*) => {
        $(
            impl View for silex_core::reactivity::Signal<$ty> {
                fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
                    crate::view::mount_reactive_view(parent, self, attrs);
                }
            }
        )*
    };
}

impl_view_for_reactive_erased_views!(
    crate::element::Element,
    crate::view::SharedView,
    crate::view::Fragment
);

impl<T: 'static> View for silex_core::reactivity::Signal<crate::element::TypedElement<T>> {
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        crate::view::mount_reactive_view(parent, self, attrs);
    }
}

macro_rules! impl_view_for_reactive_tuple_erased {
    ($($name:ident),*) => {
        impl<$($name: View + Clone + 'static),*> View for silex_core::reactivity::Signal<($($name,)*)> {
            fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
                crate::view::mount_reactive_view(parent, self, attrs);
            }
        }
    }
}

impl_view_for_reactive_tuple_erased!(A);
impl_view_for_reactive_tuple_erased!(A, B);
impl_view_for_reactive_tuple_erased!(A, B, C);
impl_view_for_reactive_tuple_erased!(A, B, C, D);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F, G);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F, G, H);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F, G, H, I);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F, G, H, I, J);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F, G, H, I, J, K);
impl_view_for_reactive_tuple_erased!(A, B, C, D, E, F, G, H, I, J, K, L);

macro_rules! impl_view_for_signal_text {
    ($($t:ty),*) => {
        $(
            impl View for silex_core::reactivity::Signal<$t> {
                fn mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
                    crate::view::mount_reactive_text(parent, self);
                }
            }
        )*
    };
}

impl_view_for_signal_text!(
    String,
    bool,
    char,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    i128,
    u128,
    isize,
    usize,
    f32,
    f64,
    &'static str,
    std::borrow::Cow<'static, str>
);

macro_rules! impl_view_forward_to_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> View for silex_core::reactivity::$ty<T>
            where
                T: silex_core::traits::RxCloneData + Sized + 'static,
                Self: silex_core::traits::IntoSignal<Value = T> + Clone + 'static,
                silex_core::reactivity::Signal<T>: View,
            {
                #[inline(always)]
                fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
                    use silex_core::traits::IntoSignal;
                    self.into_signal().mount(parent, attrs);
                }
            }
        )*
    };
}

impl_view_forward_to_signal!(ReadSignal, RwSignal, Constant, Memo);

// 为复杂响应式 Payload 转发
impl<S, F, V> View for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoSignal<Value = V> + 'static,
    V: silex_core::traits::RxCloneData + Sized + 'static,
    silex_core::reactivity::Signal<V>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoSignal;
        self.into_signal().mount(parent, attrs);
    }
}

impl<U, const N: usize> View for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoSignal<Value = U> + 'static,
    U: silex_core::traits::RxCloneData + Sized + 'static,
    silex_core::reactivity::Signal<U>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoSignal;
        self.into_signal().mount(parent, attrs);
    }
}

impl<S, F, O> View for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoSignal<Value = O> + 'static,
    O: silex_core::traits::RxCloneData + Sized + 'static,
    silex_core::reactivity::Signal<O>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoSignal;
        self.into_signal().mount(parent, attrs);
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

// 6. 元组支持
macro_rules! impl_view_for_tuple {
    ($($name:ident),*) => {
        impl<$($name: View),*> View for ($($name,)*) {
            fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
                #[allow(non_snake_case)]
                let ($($name,)*) = self;
                // Currently only apply attributes to the first element of a tuple View
                // This is a design choice consistent with Fragments.
                let mut first = true;
                $(
                    if first {
                        $name.mount(parent, attrs.clone());
                        #[allow(unused_assignments)]
                        {
                            first = false;
                        }
                    } else {
                        $name.mount(parent, Vec::new());
                    }
                )*
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
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            Ok(v) => v.mount(parent, attrs),
            Err(e) => handle_error(e),
        }
    }
}

// --- AnyView & SharedView (Type Erasure & Enum Optimization) ---

/// 辅助特征（不要求 Clone，移动语义挂载）
pub trait RenderOnce {
    fn mount_boxed(self: Box<Self>, parent: &Node, attrs: Vec<PendingAttribute>);
    fn apply_attributes_boxed(&mut self, attrs: Vec<PendingAttribute>);
}

impl<V: View + 'static> RenderOnce for V {
    fn mount_boxed(self: Box<Self>, parent: &Node, attrs: Vec<PendingAttribute>) {
        (*self).mount(parent, attrs)
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
    SharedBoxed(Box<dyn RenderShared>, Vec<PendingAttribute>),
}

/// 优化的 AnyView，作为所有视图类型擦除的终点（不要求 Clone）
#[derive(Default)]
pub enum AnyView {
    #[default]
    Empty,
    Text(String),
    Element(crate::element::Element),
    List(Vec<AnyView>),
    Unique(Box<dyn RenderOnce>, Vec<PendingAttribute>),
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
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            SharedView::Empty => {}
            SharedView::Text(s) => s.mount(parent, attrs),
            SharedView::Element(el) => el.mount(parent, attrs),
            SharedView::List(list) => {
                for (i, child) in list.into_iter().enumerate() {
                    child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
                }
            }
            SharedView::SharedBoxed(b, mut inner_attrs) => {
                inner_attrs.extend(attrs);
                b.mount_boxed(
                    parent,
                    crate::attribute::consolidate_attributes(inner_attrs),
                );
            }
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
            SharedView::SharedBoxed(_, inner_attrs) => {
                let mut temp = std::mem::take(inner_attrs);
                temp.extend(attrs);
                *inner_attrs = crate::attribute::consolidate_attributes(temp);
            }
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
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        match self {
            AnyView::Empty => {}
            AnyView::Text(s) => s.mount(parent, attrs),
            AnyView::Element(el) => el.mount(parent, attrs),
            AnyView::List(list) => {
                for (i, child) in list.into_iter().enumerate() {
                    child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
                }
            }
            AnyView::Unique(b, mut inner_attrs) => {
                inner_attrs.extend(attrs);
                b.mount_boxed(
                    parent,
                    crate::attribute::consolidate_attributes(inner_attrs),
                );
            }
            AnyView::FromShared(s) => s.mount(parent, attrs),
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
            AnyView::Unique(_, inner_attrs) => {
                let mut temp = std::mem::take(inner_attrs);
                temp.extend(attrs);
                *inner_attrs = crate::attribute::consolidate_attributes(temp);
            }
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
            SharedView::SharedBoxed(b, attrs) => {
                SharedView::SharedBoxed(b.clone_boxed(), attrs.clone())
            }
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
            Self::Unique(_, _) => write!(f, "AnyView(Unique)"),
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
            Self::SharedBoxed(_, _) => write!(f, "SharedView(SharedBoxed)"),
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
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        for (i, child) in self.0.into_iter().enumerate() {
            child.mount(parent, if i == 0 { attrs.clone() } else { Vec::new() });
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for child in &mut self.0 {
            child.apply_attributes(attrs.clone());
        }
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
impl_from_tuple!(A, B, C, D, E, F);
impl_from_tuple!(A, B, C, D, E, F, G);
impl_from_tuple!(A, B, C, D, E, F, G, H);
impl_from_tuple!(A, B, C, D, E, F, G, H, I);
impl_from_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_from_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_from_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

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
