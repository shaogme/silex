use silex_core::traits::{IntoRx, RxGet};
use silex_dom::prelude::{ApplyAttributes, AutoReactiveView, Mount, MountRef};
use web_sys::Node;

/// Show 组件：根据条件渲染不同的视图
///
/// 使用 Builder 模式构建：
/// ```rust
/// use silex::prelude::*;
///
/// let (condition, set_condition) = Signal::pair(true);
/// let view = "Content";
/// let fallback_view = "Fallback";
///
/// Show::new(condition, view)
///     .fallback(fallback_view);
/// ```
#[derive(Clone)]
pub struct Show<Cond, V, FV> {
    condition: Cond,
    view: V,
    fallback: FV,
}

// 默认无 fallback 的构造函数
impl<Cond, V> Show<Cond, V, ()>
where
    Cond: RxGet<Value = bool> + 'static,
    V: MountRef + 'static,
{
    pub fn new(condition: Cond, view: V) -> Self {
        Self {
            condition,
            view,
            fallback: (),
        }
    }
}

// Builder 方法
impl<Cond, V, FV> Show<Cond, V, FV>
where
    Cond: RxGet<Value = bool> + 'static,
    V: MountRef + 'static,
    FV: MountRef + 'static,
{
    /// 设置当条件为 false 时的 fallback 视图 (Else 分支)
    pub fn fallback<NFV>(self, fallback: NFV) -> Show<Cond, V, NFV>
    where
        NFV: MountRef + 'static,
    {
        Show {
            condition: self.condition,
            view: self.view,
            fallback,
        }
    }
}

impl<Cond, V, FV> ApplyAttributes for Show<Cond, V, FV>
where
    Cond: RxGet<Value = bool> + 'static,
    V: MountRef + 'static,
    FV: MountRef + 'static,
{
}

impl<Cond, V, FV> Mount for Show<Cond, V, FV>
where
    Cond: RxGet<Value = bool> + Clone + 'static,
    V: MountRef + Clone + 'static,
    FV: MountRef + Clone + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_show_internal(self.condition, self.view, self.fallback, parent, attrs);
    }
}

impl<Cond, V, FV> AutoReactiveView for Show<Cond, V, FV>
where
    Cond: RxGet<Value = bool> + Clone + 'static,
    V: MountRef + Clone + 'static,
    FV: MountRef + Clone + 'static,
{
}

impl<Cond, V, FV> MountRef for Show<Cond, V, FV>
where
    Cond: RxGet<Value = bool> + Clone + 'static,
    V: MountRef + Clone + 'static,
    FV: MountRef + Clone + 'static,
{
    fn mount_ref(&self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_show_internal(
            self.condition.clone(),
            self.view.clone(),
            self.fallback.clone(),
            parent,
            attrs,
        );
    }
}

fn mount_show_internal<Cond, V, FV>(
    condition: Cond,
    view: V,
    fallback: FV,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Cond: RxGet<Value = bool> + 'static,
    V: MountRef + 'static,
    FV: MountRef + 'static,
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
    fn when<V>(self, view: V) -> Show<Self::RxType, V, ()>
    where
        Self::RxType: RxGet<Value = bool> + 'static,
        V: MountRef + 'static;
}

// 为所有 IntoRx<Value = bool> 的类型实现扩展
impl<S> SignalShowExt for S
where
    S: IntoRx<Value = bool>,
{
    fn when<V>(self, view: V) -> Show<Self::RxType, V, ()>
    where
        Self::RxType: RxGet<Value = bool> + 'static,
        V: MountRef + 'static,
    {
        Show::new(self.into_rx(), view)
    }
}
