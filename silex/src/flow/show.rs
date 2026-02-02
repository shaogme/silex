use silex_core::reactivity::{Effect, ReadSignal, Signal};
use silex_core::traits::Accessor;
use silex_dom::View;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::Node;

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
/// Show::new(condition, move || view)
///     .fallback(move || fallback_view);
/// ```
#[derive(Clone)]
pub struct Show<Cond, ViewFn, FalsyViewFn, V1, V2> {
    condition: Cond,
    view: ViewFn,
    fallback: FalsyViewFn,
    _marker: std::marker::PhantomData<(V1, V2)>,
}

// 默认无 fallback 的构造函数
impl<Cond, ViewFn, V1> Show<Cond, ViewFn, fn() -> (), V1, ()>
where
    Cond: Accessor<Value = bool> + 'static,
    ViewFn: Fn() -> V1 + 'static,
    V1: View,
{
    pub fn new(condition: Cond, view: ViewFn) -> Self {
        Self {
            condition,
            view,
            fallback: || (),
            _marker: std::marker::PhantomData,
        }
    }
}

// Builder 方法
impl<Cond, ViewFn, FalsyViewFn, V1, V2> Show<Cond, ViewFn, FalsyViewFn, V1, V2>
where
    Cond: Accessor<Value = bool> + 'static,
    ViewFn: Fn() -> V1 + 'static,
    FalsyViewFn: Fn() -> V2 + 'static,
    V1: View,
    V2: View,
{
    /// 设置当条件为 false 时的 fallback 视图 (Else 分支)
    pub fn fallback<NewFalsyFn, NewV2>(
        self,
        fallback: NewFalsyFn,
    ) -> Show<Cond, ViewFn, NewFalsyFn, V1, NewV2>
    where
        NewFalsyFn: Fn() -> NewV2 + 'static,
        NewV2: View,
    {
        Show {
            condition: self.condition,
            view: self.view,
            fallback,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<Cond, ViewFn, FalsyViewFn, V1, V2> View for Show<Cond, ViewFn, FalsyViewFn, V1, V2>
where
    Cond: Accessor<Value = bool> + 'static,
    ViewFn: Fn() -> V1 + 'static,
    FalsyViewFn: Fn() -> V2 + 'static,
    V1: View,
    V2: View,
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
            let val = cond.value();

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
                (view_fn)().mount(&fragment_node);
            } else {
                (fallback_fn)().mount(&fragment_node);
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

/// Signal 扩展特质，提供 .when() 语法糖
pub trait SignalShowExt {
    // 使用 Box<dyn> 简化返回类型签名
    type Cond: Accessor<Value = bool> + 'static;

    fn when<V, F>(self, view: F) -> Show<Self::Cond, F, fn() -> (), V, ()>
    where
        V: View,
        F: Fn() -> V + 'static;
}

// 为 ReadSignal<bool> 实现扩展
impl SignalShowExt for ReadSignal<bool> {
    type Cond = Self;

    fn when<V, F>(self, view: F) -> Show<Self::Cond, F, fn() -> (), V, ()>
    where
        V: View,
        F: Fn() -> V + 'static,
    {
        Show::new(self, view)
    }
}

// 为 Memo<bool> 实现扩展
impl SignalShowExt for silex_core::reactivity::Memo<bool> {
    type Cond = Self;

    fn when<V, F>(self, view: F) -> Show<Self::Cond, F, fn() -> (), V, ()>
    where
        V: View,
        F: Fn() -> V + 'static,
    {
        Show::new(self, view)
    }
}

// 为 Signal<bool> 实现扩展
impl SignalShowExt for Signal<bool> {
    type Cond = Self;

    fn when<V, F>(self, view: F) -> Show<Self::Cond, F, fn() -> (), V, ()>
    where
        V: View,
        F: Fn() -> V + 'static,
    {
        Show::new(self, view)
    }
}
