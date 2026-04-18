use silex_core::reactivity::Effect;
use silex_core::traits::{RxGet, RxRead};
use silex_dom::prelude::View;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::Node;

/// Helper trait to deduce View type from a reactive view factory
pub trait ViewFactory {
    type View: View;
    fn render(&self) -> Self::View;
}

impl<V, M> ViewFactory for silex_core::Rx<V, M>
where
    V: View + Clone + 'static,
    M: 'static,
{
    type View = V;
    fn render(&self) -> Self::View {
        self.get_untracked()
    }
}

/// Implement ViewFactory for empty function pointer to support default fallback
impl ViewFactory for fn() -> () {
    type View = ();
    fn render(&self) -> Self::View {}
}

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
/// Show::new(condition, rx!(view))
///     .fallback(rx!(fallback_view));
/// ```
pub struct Show<Cond, ViewFn, FalsyViewFn> {
    condition: Cond,
    view: Rc<ViewFn>,
    fallback: Rc<FalsyViewFn>,
}

impl<Cond: Clone, ViewFn, FalsyViewFn> Clone for Show<Cond, ViewFn, FalsyViewFn> {
    fn clone(&self) -> Self {
        Self {
            condition: self.condition.clone(),
            view: self.view.clone(),
            fallback: self.fallback.clone(),
        }
    }
}

// 默认无 fallback 的构造函数
impl<Cond, ViewFn> Show<Cond, ViewFn, fn() -> ()> {
    pub fn new(condition: Cond, view: ViewFn) -> Self
    where
        Cond: RxRead<Value = bool> + 'static,
        ViewFn: ViewFactory + 'static,
    {
        Self {
            condition,
            view: Rc::new(view),
            fallback: Rc::new(|| ()),
        }
    }
}

// Builder 方法
impl<Cond, ViewFn, FalsyViewFn> Show<Cond, ViewFn, FalsyViewFn>
where
    Cond: RxRead<Value = bool> + 'static,
    ViewFn: ViewFactory + 'static,
    FalsyViewFn: ViewFactory + 'static,
{
    /// 设置当条件为 false 时的 fallback 视图 (Else 分支)
    pub fn fallback<NewFalsyFn>(self, fallback: NewFalsyFn) -> Show<Cond, ViewFn, NewFalsyFn>
    where
        NewFalsyFn: ViewFactory + 'static,
    {
        Show {
            condition: self.condition,
            view: self.view,
            fallback: Rc::new(fallback),
        }
    }
}

impl<Cond, ViewFn, FalsyViewFn> View for Show<Cond, ViewFn, FalsyViewFn>
where
    Cond: RxGet<Value = bool> + Clone + 'static,
    ViewFn: ViewFactory + 'static,
    FalsyViewFn: ViewFactory + 'static,
{
    fn mount(self, parent: &Node, attrs: Vec<silex_dom::attribute::PendingAttribute>) {
        mount_show_internal(self.condition, self.view, self.fallback, parent, attrs);
    }

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

fn mount_show_internal<Cond, ViewFn, FalsyViewFn>(
    condition: Cond,
    view: Rc<ViewFn>,
    fallback: Rc<FalsyViewFn>,
    parent: &Node,
    attrs: Vec<silex_dom::attribute::PendingAttribute>,
) where
    Cond: RxGet<Value = bool> + 'static,
    ViewFn: ViewFactory + 'static,
    FalsyViewFn: ViewFactory + 'static,
{
    let document = silex_dom::document();

    let start_node: Node = document.create_comment("show-start").into();
    let _ = parent.append_child(&start_node);

    let end_node: Node = document.create_comment("show-end").into();
    let _ = parent.append_child(&end_node);

    let prev_state = Rc::new(RefCell::new(None::<bool>));

    Effect::new(move |_| {
        let val = condition.get();
        let mut state = prev_state.borrow_mut();
        if *state == Some(val) {
            return;
        }

        if let Some(parent) = start_node.parent_node() {
            while let Some(sibling) = start_node.next_sibling() {
                if sibling == end_node {
                    break;
                }
                let _ = parent.remove_child(&sibling);
            }
        }

        let fragment_node: Node = document.create_document_fragment().into();
        if val {
            view.render().mount(&fragment_node, attrs.clone());
        } else {
            fallback.render().mount(&fragment_node, attrs.clone());
        }

        if let Some(parent) = end_node.parent_node() {
            let _ = parent.insert_before(&fragment_node, Some(&end_node));
        }

        *state = Some(val);
    });
}

// --- Signal 扩展 ---

use silex_core::traits::IntoRx;

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt: IntoRx<Value = bool> {
    fn when<F>(self, view: F) -> Show<Self::RxType, F, fn() -> ()>
    where
        Self::RxType: RxRead<Value = bool> + 'static,
        F: ViewFactory + 'static;
}

// 为所有 IntoRx<Value = bool> 的类型实现扩展
impl<S> SignalShowExt for S
where
    S: IntoRx<Value = bool>,
{
    fn when<F>(self, view: F) -> Show<Self::RxType, F, fn() -> ()>
    where
        Self::RxType: RxRead<Value = bool> + 'static,
        F: ViewFactory + 'static,
    {
        Show::new(self.into_rx(), view)
    }
}
