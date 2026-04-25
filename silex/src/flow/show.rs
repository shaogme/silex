use silex_core::traits::{IntoRx, RxGet};
use silex_dom::prelude::*;
use silex_macros::component;

/// Show 组件：根据条件渲染不同的视图
///
/// 使用方式：
/// ```rust
/// Show(condition).children(view).fallback(fallback_view)
/// ```
#[component]
pub fn Show<C>(
    when: C,
    #[prop(render)]
    #[chain]
    children: AnyView,
    #[prop(render)]
    #[chain(default = AnyView::Empty)]
    fallback: AnyView,
) -> impl View
where
    C: RxGet<Value = bool> + Clone + 'static,
{
    silex_core::rx! {
        if when.get() {
            children.clone()
        } else {
            fallback.clone()
        }
    }
}

// --- Signal 扩展 ---

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt: IntoRx<Value = bool> {
    fn when<V>(self, view: V) -> ShowComponent<Self::RxType>
    where
        Self::RxType: RxGet<Value = bool> + Clone + 'static,
        V: View + 'static;
}

// 为所有 IntoRx<Value = bool> 的类型实现扩展
impl<S> SignalShowExt for S
where
    S: IntoRx<Value = bool>,
{
    fn when<V>(self, view: V) -> ShowComponent<Self::RxType>
    where
        Self::RxType: RxGet<Value = bool> + Clone + 'static,
        V: View + 'static,
    {
        Show(self.into_rx()).children(view)
    }
}
