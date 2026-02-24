use silex_core::reactivity::Effect;
use silex_core::traits::{Read, RxInternal};
use silex_dom::prelude::View;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;
use web_sys::Node;

/// Helper trait to deduce View type from a reactive view factory
pub trait ViewFactory {
    type View: View;
    fn render(&self) -> Self::View;
}

impl<F, V, M> ViewFactory for silex_core::Rx<F, M>
where
    F: Fn() -> V,
    V: View,
{
    type View = V;
    fn render(&self) -> Self::View {
        (self.0)()
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
/// let (condition, set_condition) = signal(true);
/// let view = "Content";
/// let fallback_view = "Fallback";
///
/// Show::new(condition, rx!(view))
///     .fallback(rx!(fallback_view));
/// ```
#[derive(Clone)]
pub struct Show<Cond, ViewFn, FalsyViewFn> {
    condition: Cond,
    view: ViewFn,
    fallback: FalsyViewFn,
}

// 默认无 fallback 的构造函数
impl<Cond, ViewFn> Show<Cond, ViewFn, fn() -> ()> {
    pub fn new(condition: Cond, view: ViewFn) -> Self
    where
        Cond: Read<Value = bool> + 'static,
        for<'a> Cond::ReadOutput<'a>: Deref<Target = bool>,
        ViewFn: ViewFactory + 'static,
    {
        Self {
            condition,
            view,
            fallback: || (),
        }
    }
}

// Builder 方法
impl<Cond, ViewFn, FalsyViewFn> Show<Cond, ViewFn, FalsyViewFn>
where
    Cond: Read<Value = bool> + 'static,
    for<'a> Cond::ReadOutput<'a>: Deref<Target = bool>,
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
            fallback,
        }
    }
}

impl<Cond, ViewFn, FalsyViewFn> View for Show<Cond, ViewFn, FalsyViewFn>
where
    Cond: Read<Value = bool> + 'static,
    for<'a> Cond::ReadOutput<'a>: Deref<Target = bool>,
    ViewFn: ViewFactory + Clone + 'static,
    FalsyViewFn: ViewFactory + Clone + 'static,
{
    fn mount(self, parent: &Node) {
        let document = silex_dom::document();

        // 1. Create Anchors (Start & End Markers)
        let start_marker = document.create_comment("show-start");
        let start_node: Node = start_marker.into();

        if let Err(e) = parent
            .append_child(&start_node)
            .map_err(crate::SilexError::from)
        {
            silex_core::error::handle_error(e);
            return;
        }

        let end_marker = document.create_comment("show-end");
        let end_node: Node = end_marker.into();

        if let Err(e) = parent
            .append_child(&end_node)
            .map_err(crate::SilexError::from)
        {
            silex_core::error::handle_error(e);
            return;
        }

        let cond = self.condition;
        let view_fn = self.view;
        let fallback_fn = self.fallback;
        let prev_state = Rc::new(RefCell::new(None::<bool>));

        Effect::new(move |_| {
            let val = cond.get();

            let mut state = prev_state.borrow_mut();

            if *state == Some(val) {
                return;
            }

            // 清理旧内容 (Clean up between markers)
            if let Some(parent) = start_node.parent_node() {
                while let Some(sibling) = start_node.next_sibling() {
                    if sibling == end_node {
                        break;
                    }
                    let _ = parent.remove_child(&sibling);
                }
            }

            // 准备新内容 (Prepare new content)
            let fragment = document.create_document_fragment();
            let fragment_node: Node = fragment.clone().into();

            if val {
                view_fn.render().mount(&fragment_node);
            } else {
                fallback_fn.render().mount(&fragment_node);
            }

            // 插入新内容 (Insert new content)
            if let Some(parent) = end_node.parent_node() {
                let _ = parent.insert_before(&fragment_node, Some(&end_node));
            }

            *state = Some(val);
        });
    }
}

// --- Signal 扩展 ---

use silex_core::traits::IntoRx;

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt: IntoRx<Value = bool> {
    fn when<F>(self, view: F) -> Show<Self::RxType, F, fn() -> ()>
    where
        Self::RxType: Read<Value = bool> + 'static,
        for<'a> <Self::RxType as RxInternal>::ReadOutput<'a>: Deref<Target = bool>,
        F: ViewFactory + 'static;
}

// 为所有 IntoRx<Value = bool> 的类型实现扩展
impl<S> SignalShowExt for S
where
    S: IntoRx<Value = bool>,
{
    fn when<F>(self, view: F) -> Show<Self::RxType, F, fn() -> ()>
    where
        Self::RxType: Read<Value = bool> + 'static,
        for<'a> <S::RxType as RxInternal>::ReadOutput<'a>: Deref<Target = bool>,
        F: ViewFactory + 'static,
    {
        Show::new(self.into_rx(), view)
    }
}
