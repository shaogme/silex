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
    #[prop(into)] children: SharedView,
    #[prop(default = SharedView::Empty, into)] fallback: SharedView,
) -> impl Mount + MountRef
where
    C: RxGet<Value = bool> + Clone + 'static,
{
    ShowView {
        when: when.clone(),
        children: children.clone(),
        fallback: fallback.clone(),
    }
}

#[derive(Clone)]
struct ShowView<C> {
    when: C,
    children: SharedView,
    fallback: SharedView,
}

impl<C> ApplyAttributes for ShowView<C> {}

impl<C> Mount for ShowView<C>
where
    C: RxGet<Value = bool> + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_show_internal(self.when, self.children, self.fallback, parent, attrs);
    }
}

impl<C> AutoReactiveView for ShowView<C> where C: RxGet<Value = bool> + Clone + 'static {}

impl<C> MountRef for ShowView<C>
where
    C: RxGet<Value = bool> + Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_show_internal(
            self.when.clone(),
            self.children.clone(),
            self.fallback.clone(),
            parent,
            attrs,
        );
    }
}

fn mount_show_internal<C>(
    condition: C,
    view: SharedView,
    fallback: SharedView,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    C: RxGet<Value = bool> + 'static,
{
    use silex_dom::view::any::RenderThunk;
    silex_dom::view::mount_dynamic_view_universal(
        parent,
        attrs,
        RenderThunk::new(move |args| {
            let (p, a) = args;
            if condition.get() {
                view.mount_ref(&p, a);
            } else {
                fallback.mount_ref(&p, a);
            }
        }),
    );
}

// --- Signal 扩展 ---

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt: IntoRx<Value = bool> {
    fn when<V>(self, view: V) -> ShowComponent<Self::RxType>
    where
        Self::RxType: RxGet<Value = bool> + Clone + 'static,
        V: MountRefExt + 'static;
}

// 为所有 IntoRx<Value = bool> 的类型实现扩展
impl<S> SignalShowExt for S
where
    S: IntoRx<Value = bool>,
{
    fn when<V>(self, view: V) -> ShowComponent<Self::RxType>
    where
        Self::RxType: RxGet<Value = bool> + Clone + 'static,
        V: MountRefExt + 'static,
    {
        Show(self.into_rx()).children(view.into_shared())
    }
}
