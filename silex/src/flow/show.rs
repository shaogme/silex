use silex_core::traits::{IntoRx, RxGet};
use silex_dom::prelude::*;
use silex_macros::component;
use web_sys::Node;

/// Show 组件：根据条件渲染不同的视图
///
/// 使用方式：
/// ```rust
/// Show(condition).children(view).fallback(fallback_view)
/// ```
#[component]
pub fn Show<C>(
    when: C,
    #[prop(render)] children: AnyView,
    #[prop(default = AnyView::Empty, render)] fallback: AnyView,
) -> impl View
where
    C: RxGet<Value = bool> + Clone + 'static,
{
    ShowView {
        when,
        children,
        fallback,
    }
}

#[derive(Clone)]
struct ShowView<'a, C> {
    when: Prop<'a, C>,
    children: Prop<'a, AnyView>,
    fallback: Prop<'a, AnyView>,
}

impl<'a, C> ApplyAttributes for ShowView<'a, C> {}

impl<'a, C> View for ShowView<'a, C>
where
    C: RxGet<Value = bool> + Clone + 'static,
{
    fn mount(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_show_internal(
            Prop::new_owned(self.when.clone()),
            Prop::new_owned(self.children.clone()),
            Prop::new_owned(self.fallback.clone()),
            parent,
            attrs,
        );
    }

    fn mount_owned(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>)
    where
        Self: Sized,
    {
        mount_show_internal(self.when, self.children, self.fallback, parent, attrs);
    }
}

fn mount_show_internal<'a, C>(
    condition: Prop<'a, C>,
    view: Prop<'a, AnyView>,
    fallback: Prop<'a, AnyView>,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    C: RxGet<Value = bool> + Clone + 'static,
{
    let condition = condition.into_owned();
    let view = view.into_owned();
    let fallback = fallback.into_owned();
    silex_dom::view::mount_branch_cached(
        parent,
        attrs,
        move || condition.get(),
        move |active| {
            if active {
                view.clone()
            } else {
                fallback.clone()
            }
        },
    );
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
