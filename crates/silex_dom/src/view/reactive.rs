use crate::attribute::PendingAttribute;
use crate::element::{Element, TypedElement, tags::Tag};
use crate::view::{
    ApplyAttributes, View, ViewCons,
    any::{AnyView, RenderThunk},
    mount_dynamic_view_universal,
};
use silex_core::error::handle_error;
use silex_core::reactivity::{
    Constant, DerivedPayload, Effect, Memo, ReadSignal, RwSignal, Signal, SignalSlice,
};
use silex_core::traits::{IntoRx, RxCloneData, RxRead};
use silex_core::{Rx, RxValueKind, SilexError};
use std::borrow::Cow;
use std::fmt::Display;
use std::rc::Rc;
use web_sys::Node;

// --- 响应式文本归一化内核 (Reactive Text Consolidation Kernel) ---

/// 泛型内核函数：负责将任何响应式类型转换为文本视图。
pub(crate) fn mount_reactive_text<T, M>(parent: &Node, rx: Rx<T, M>)
where
    T: Display + RxCloneData,
    M: 'static,
{
    let document = crate::document();
    let node = document.create_text_node("");
    if let Err(e) = parent.append_child(&node).map_err(SilexError::from) {
        handle_error(e);
        return;
    }

    Effect::new(move |_| {
        // 直接读取原始信号。
        // Silex 调度系统会确保当 Effect 或其 Parent 为 Inert 时不执行此闭包。
        rx.with(|value| {
            node.set_node_value(Some(&value.to_string()));
        });
    });
}

// --- 响应式组件视图内核 (Reactive View Core) ---

pub(crate) fn mount_reactive_view<V, M>(parent: &Node, rx: Rx<V, M>, attrs: Vec<PendingAttribute>)
where
    V: View + 'static,
    M: 'static,
{
    mount_dynamic_view_universal(
        parent,
        attrs,
        RenderThunk::new(move |args| {
            let (p, a) = args;
            rx.with(|view| view.mount(&p, a))
        }),
    );
}

// 4. Rx wrapper support (Unified entry point for reactive normalization)

impl<V, M> ApplyAttributes for Rx<V, M>
where
    V: RxCloneData,
    M: 'static,
    Self: RxViewDispatcher,
{
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}
}

impl<V, M> View for Rx<V, M>
where
    V: Sized,
    M: 'static,
    Self: RxViewDispatcher,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        (*self).dispatch_mount(parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        self.dispatch_mount(parent, attrs);
    }
}

/// 内部特征，用于 Rx 的 View 分发，解决 trait 冲突并优化路径。
///
/// 任何希望作为 `Rx<V>` 挂载的视图类型，应实现 `AutoReactiveView`。
pub trait RxViewDispatcher {
    fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>);
}

/// 核心特征：自动响应式视图。
///
/// 实现此特征的类型 `V` 会自动让 `Rx<V>` 获得视图挂载能力。
/// 这是解决跨 Crate 的响应式组件支持的最佳方案。
pub trait AutoReactiveView: View + Sized + 'static {
    /// 响应式挂载策略。默认使用 `mount_reactive_view`（完全重新挂载分支）。
    /// 对于 `String` 等基础类型，应重写此方法以改用高效的 `mount_reactive_text`。
    fn mount_reactive<M: 'static>(rx: Rx<Self, M>, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_reactive_view(parent, rx, attrs);
    }
}

// 统一的分发器实现
impl<V: AutoReactiveView, M: 'static> RxViewDispatcher for Rx<V, M> {
    fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        V::mount_reactive(self, parent, attrs);
    }
}

macro_rules! impl_auto_reactive_view_text {
    ($($t:ty),*) => {
        $(
            impl AutoReactiveView for $t {
                #[inline(always)]
                fn mount_reactive<M: 'static>(rx: Rx<Self, M>, parent: &Node, _attrs: Vec<PendingAttribute>) {
                    mount_reactive_text(parent, rx);
                }
            }
        )*
    };
}

macro_rules! impl_auto_reactive_view_default {
    ($($t:ty),*) => {
        $(
            impl AutoReactiveView for $t {}
        )*
    };
}

impl_auto_reactive_view_text!(
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
    Cow<'static, str>
);

impl_auto_reactive_view_default!(Element, AnyView);

impl<V: View + 'static> AutoReactiveView for Option<V> {}

impl<H, T> AutoReactiveView for ViewCons<H, T>
where
    H: View + 'static,
    T: View + 'static,
{
}

impl<T: Tag + 'static> AutoReactiveView for TypedElement<T> {}

// --- Signal 自动支持宏 ---

macro_rules! impl_view_forward_to_rx {
    ($($ty:ident),*) => {
        $(
            impl<T> ApplyAttributes for $ty<T>
            where
                T: RxCloneData,
                Self: IntoRx<RxType = Rx<T, RxValueKind>> + Clone,
                Rx<T, RxValueKind>: ApplyAttributes,
            {}

            impl<T> View for $ty<T>
            where
                T: RxCloneData,
                Self: IntoRx<RxType = Rx<T, RxValueKind>> + Clone,
                Rx<T, RxValueKind>: View,
            {
                fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
                    self.clone().into_rx().mount(parent, attrs);
                }

                fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
                where
                    Self: Sized,
                {
                    self.into_rx().mount_owned(parent, attrs);
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, RwSignal, Constant, Memo, Signal);

impl<S, F, V> ApplyAttributes for DerivedPayload<S, F>
where
    Self: IntoRx<RxType = Rx<V, RxValueKind>> + Clone,
    V: RxCloneData,
    Rx<V, RxValueKind>: ApplyAttributes,
{
}

impl<S, F, V> View for DerivedPayload<S, F>
where
    Self: IntoRx<RxType = Rx<V, RxValueKind>> + Clone,
    V: RxCloneData,
    Rx<V, RxValueKind>: View,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        self.clone().into_rx().mount(parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        self.into_rx().mount_owned(parent, attrs);
    }
}

impl<S, F, V> ApplyAttributes for Rc<DerivedPayload<S, F>>
where
    Self: IntoRx<RxType = Rx<V, RxValueKind>>,
    V: RxCloneData,
    Rx<V, RxValueKind>: ApplyAttributes,
{
}

impl<S, F, V> View for Rc<DerivedPayload<S, F>>
where
    Self: IntoRx<RxType = Rx<V, RxValueKind>>,
    V: RxCloneData,
    Rx<V, RxValueKind>: View,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        self.clone().into_rx().mount(parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        self.into_rx().mount_owned(parent, attrs);
    }
}

impl<S, F, O> ApplyAttributes for SignalSlice<S, F, O>
where
    Self: IntoRx<RxType = Rx<O, RxValueKind>> + Clone,
    O: RxCloneData,
    Rx<O, RxValueKind>: ApplyAttributes,
{
}

impl<S, F, O> View for SignalSlice<S, F, O>
where
    Self: IntoRx<RxType = Rx<O, RxValueKind>> + Clone,
    O: RxCloneData,
    Rx<O, RxValueKind>: View,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        self.clone().into_rx().mount(parent, attrs);
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        self.into_rx().mount_owned(parent, attrs);
    }
}
