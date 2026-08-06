use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead};
use silex_dom::prelude::*;
use silex_macros::component;
use std::marker::PhantomData;
use std::rc::Rc;

pub trait IndexChildren<'scope, Item> {
    type View: View<'scope> + 'scope;

    fn render(&self, item: Item, index: usize) -> Self::View;
}

impl<'scope, Item, F, V> IndexChildren<'scope, Item> for F
where
    F: Fn(Item, usize) -> V,
    V: View<'scope> + 'scope,
{
    type View = V;

    fn render(&self, item: Item, index: usize) -> Self::View {
        self(item, index)
    }
}

/// Index 组件：类似于 For，但基于索引（Index）进行迭代。
///
/// 当列表顺序发生变化时，DOM 节点不会移动，只是对应的数据 Signal 会更新。
/// 适用于基础类型列表或无唯一 Key 的列表。
///
/// 使用方式：
/// ```rust,ignore
/// Index(list).children(|item, index| li(rx! { index.get() }))
/// ```
#[component]
pub fn Index<'scope, IF, I, IS, MF>(
    each: IF,
    #[prop(render)]
    #[chain]
    children: MF,
    #[chain(default)] _scope: PhantomData<&'scope ()>,
) -> silex_dom::view::list::IndexedLoopView<'scope, IF, I, IS>
where
    IF: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = I> + 'scope,
    MF: IndexChildren<'scope, I> + Clone + 'scope,
    I: Clone + 'scope,
{
    let view_fn = Rc::new(move |item: I, index: usize| children.render(item, index).into_any());

    silex_dom::view::list::IndexedLoopView {
        each,
        view_fn,
        _marker: std::marker::PhantomData,
    }
}
