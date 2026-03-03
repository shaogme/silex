use crate::attribute::PendingAttribute;
use crate::view::View;
use silex_core::error::handle_error;
use silex_core::reactivity::Effect;
use silex_core::traits::{RxCloneData, RxRead};
use silex_core::{Rx, RxValueKind, SilexError};
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
    V: View + RxCloneData + 'static,
    M: 'static,
{
    crate::view::mount_dynamic_view_universal(
        parent,
        attrs,
        crate::view::ViewThunk::new(move || rx.with(|view| view.clone().into_any())),
    );
}

// 4. Rx wrapper support (Unified entry point for reactive normalization)
impl<V, M> View for Rx<V, M>
where
    V: RxCloneData + Sized + 'static,
    M: 'static,
    Self: RxViewDispatcher,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        self.dispatch_mount(parent, attrs);
    }

    fn apply_attributes(&mut self, _attrs: Vec<PendingAttribute>) {}
}

/// 内部特征，用于 Rx 的 View 分发，解决 trait 冲突并优化路径
pub trait RxViewDispatcher {
    fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>);
}

macro_rules! impl_rx_view_dispatcher_text {
    ($($t:ty),*) => {
        $(
            impl<M: 'static> RxViewDispatcher for Rx<$t, M> {
                fn dispatch_mount(self, parent: &Node, _attrs: Vec<PendingAttribute>) {
                    mount_reactive_text(parent, self);
                }
            }
        )*
    };
}

macro_rules! impl_rx_view_dispatcher_view {
    ($($t:ty),*) => {
        $(
            impl<M: 'static> RxViewDispatcher for Rx<$t, M> {
                fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
                    mount_reactive_view(parent, self, attrs);
                }
            }
        )*
    };
}

impl_rx_view_dispatcher_text!(
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

impl_rx_view_dispatcher_view!(
    crate::element::Element,
    crate::view::SharedView,
    crate::view::any::Fragment
);

impl<V: View + RxCloneData + 'static, M: 'static> RxViewDispatcher for Rx<Option<V>, M> {
    fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_reactive_view(parent, self, attrs);
    }
}

// --- Recursive View Chain Dispatcher ---

impl<H, T, M> RxViewDispatcher for silex_core::Rx<crate::view::ViewCons<H, T>, M>
where
    H: View + Clone + 'static,
    T: View + Clone + 'static,
    M: 'static,
{
    fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_reactive_view(parent, self, attrs);
    }
}

impl<T: 'static, M: 'static> RxViewDispatcher for Rx<crate::element::TypedElement<T>, M> {
    fn dispatch_mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        mount_reactive_view(parent, self, attrs);
    }
}

macro_rules! impl_view_forward_to_rx {
    ($($ty:ident),*) => {
        $(
            impl<T> View for silex_core::reactivity::$ty<T>
            where
                T: RxCloneData + Sized + 'static,
                Self: silex_core::traits::IntoRx<RxType = Rx<T, RxValueKind>> + Clone + 'static,
                Rx<T, RxValueKind>: View,
            {
                #[inline(always)]
                fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
                    use silex_core::traits::IntoRx;
                    self.into_rx().mount(parent, attrs);
                }
            }
        )*
    };
}

impl_view_forward_to_rx!(ReadSignal, RwSignal, Constant, Memo, Signal);

impl<S, F, V> View for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<V, RxValueKind>> + 'static,
    V: RxCloneData + Sized + 'static,
    Rx<V, RxValueKind>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoRx;
        self.into_rx().mount(parent, attrs);
    }
}

impl<U, const N: usize> View for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<U, RxValueKind>> + 'static,
    U: RxCloneData + Sized + 'static,
    Rx<U, RxValueKind>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoRx;
        self.into_rx().mount(parent, attrs);
    }
}

impl<S, F, O> View for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx<RxType = Rx<O, RxValueKind>> + 'static,
    O: RxCloneData + Sized + 'static,
    Rx<O, RxValueKind>: View,
{
    #[inline(always)]
    fn mount(self, parent: &Node, attrs: Vec<PendingAttribute>) {
        use silex_core::traits::IntoRx;
        self.into_rx().mount(parent, attrs);
    }
}
