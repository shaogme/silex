use crate::attribute::PendingAttribute;
use silex_core::error::handle_error;
use silex_core::reactivity::Effect;
use silex_core::traits::{IntoRx, RxCloneData, RxRead};
use silex_core::{Rx, SilexError};
use std::fmt::Display;
use web_sys::Node;

// --- 响应式文本归一化内核 (Reactive Text Consolidation Kernel) ---

/// 泛型内核函数：负责将任何响应式类型转换为文本视图。
pub(crate) fn mount_reactive_text<T, M>(parent: &Node, rx: Rx<T, M>)
where
    T: Display + RxCloneData + 'static,
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
    V: crate::view::View + 'static,
    M: 'static,
{
    crate::view::mount_dynamic_view_universal(
        parent,
        attrs,
        crate::view::any::RenderThunk::new(move |args| {
            let (p, a) = args;
            rx.with(|view| view.mount(&p, a))
        }),
    );
}

// 4. Rx wrapper support (Unified entry point for reactive normalization)

impl<V, M> crate::view::ApplyAttributes for Rx<V, M>
where
    V: RxCloneData + Sized + 'static,
    M: 'static,
    Self: RxViewDispatcher,
{
    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}
}

impl<V, M> crate::view::View for Rx<V, M>
where
    V: Sized + 'static,
    M: 'static,
    Self: RxViewDispatcher,
{
    fn mount(&self, parent: &Node, attrs: Vec<PendingAttribute>) {
        self.clone().dispatch_mount(parent, attrs);
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
pub trait AutoReactiveView: crate::view::View + Sized + 'static {
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
    std::borrow::Cow<'static, str>
);

impl_auto_reactive_view_default!(crate::element::Element, crate::view::any::AnyView);

impl<V: crate::view::View + 'static> AutoReactiveView for Option<V> {}

impl<H, T> AutoReactiveView for crate::view::ViewCons<H, T>
where
    H: crate::view::View + 'static,
    T: crate::view::View + 'static,
{
}

impl<T: 'static> AutoReactiveView for crate::element::TypedElement<T> {}

// --- Signal 自动支持宏 ---

use silex_core::RxValueKind;

macro_rules! impl_view_forward_to_rx {
    ($($ty:ident),*) => {
        $(
            impl<T> crate::view::ApplyAttributes for silex_core::reactivity::$ty<T>
            where
                T: RxCloneData + Sized + 'static,
                Self: silex_core::traits::IntoRx<RxType = Rx<T, RxValueKind>> + Clone + 'static,
                Rx<T, RxValueKind>: crate::view::ApplyAttributes,
            {}

            impl<T> crate::view::View for silex_core::reactivity::$ty<T>
            where
                T: RxCloneData + Sized + 'static,
                Self: silex_core::traits::IntoRx<RxType = Rx<T, RxValueKind>> + Clone + 'static,
                Rx<T, RxValueKind>: crate::view::View,
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

impl<S, F, V> crate::view::ApplyAttributes for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<V, RxValueKind>> + Clone + 'static,
    V: RxCloneData + Sized + 'static,
    Rx<V, RxValueKind>: crate::view::ApplyAttributes,
{
}

impl<S, F, V> crate::view::View for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<V, RxValueKind>> + Clone + 'static,
    V: RxCloneData + Sized + 'static,
    Rx<V, RxValueKind>: crate::view::View,
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

impl<S, F, V> crate::view::ApplyAttributes
    for std::rc::Rc<silex_core::reactivity::DerivedPayload<S, F>>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<V, RxValueKind>> + 'static,
    V: RxCloneData + Sized + 'static,
    Rx<V, RxValueKind>: crate::view::ApplyAttributes,
{
}

impl<S, F, V> crate::view::View for std::rc::Rc<silex_core::reactivity::DerivedPayload<S, F>>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<V, RxValueKind>> + 'static,
    V: RxCloneData + Sized + 'static,
    Rx<V, RxValueKind>: crate::view::View,
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

impl<S, F, O> crate::view::ApplyAttributes for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<O, RxValueKind>> + Clone + 'static,
    O: RxCloneData + Sized + 'static,
    Rx<O, RxValueKind>: crate::view::ApplyAttributes,
{
}

impl<S, F, O> crate::view::View for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<O, RxValueKind>> + Clone + 'static,
    O: RxCloneData + Sized + 'static,
    Rx<O, RxValueKind>: crate::view::View,
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
