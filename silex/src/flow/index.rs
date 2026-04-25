use silex_core::reactivity::ReadSignal;
use silex_core::traits::{ForLoopSource, RxRead};
use silex_dom::prelude::*;
use silex_macros::component;
use std::rc::Rc;

/// Index 组件：类似于 For，但基于索引（Index）进行迭代。
///
/// 当列表顺序发生变化时，DOM 节点不会移动，只是对应的数据 Signal 会更新。
/// 适用于基础类型列表或无唯一 Key 的列表。
///
/// 使用方式：
/// ```rust
/// Index(list).children(|item, index| li(rx! { index.get() }))
/// ```
#[component]
pub fn Index<IF, I, IS, MF, V>(
    each: IF,
    #[prop(render)]
    #[chain]
    children: MF,
) -> impl View
where
    IF: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = I> + 'static,
    MF: Fn(ReadSignal<I>, ReadSignal<usize>) -> V + Clone + 'static,
    V: View + 'static,
    I: Clone + 'static,
{
    let view_fn = Rc::new(move |item: ReadSignal<I>, index: ReadSignal<usize>| {
        children(item, index).into_any()
    });

    silex_dom::view::list::IndexedLoopView {
        each,
        view_fn,
        _marker: std::marker::PhantomData,
    }
}
