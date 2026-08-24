use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use silex_macros::component;
use silex_view::AnyView;
use silex_view::prelude::*;
use std::marker::PhantomData;
use std::rc::Rc;

/// 类型擦除的按索引列表行渲染器。
#[derive(Clone)]
pub struct IndexRenderer<'scope, Item: 'scope> {
    render: Rc<dyn Fn(Item, usize) -> AnyView<'scope> + 'scope>,
}

impl<'scope, Item: 'scope> IndexRenderer<'scope, Item> {
    pub fn from_fn<F, V>(render: F) -> Self
    where
        F: Fn(Item, usize) -> V + 'scope,
        V: View<'scope> + 'scope,
    {
        Self {
            render: Rc::new(move |item, index| render(item, index).into_any()),
        }
    }

    fn render(&self, item: Item, index: usize) -> AnyView<'scope> {
        (self.render)(item, index)
    }
}

/// `Index` 组件：类似于 `For`，但基于索引进行迭代。
///
/// 当列表顺序发生变化时，DOM 节点不会移动，只是对应的数据 Signal 会更新。
/// 适用于基础类型列表或无唯一 Key 的列表。
///
/// ```rust,ignore
/// Index(list)
///     .children(|item, index| li(format!("{}: {}", index, item.name)))
///     .build()
/// ```
#[component]
pub fn Index<'scope, Ctx, IF, I, IS>(
    #[ctx] ctx: Ctx,
    each: IF,
    #[prop(render_fn(I, usize))]
    #[chain]
    children: IndexRenderer<'scope, I>,
    #[chain(default)] _scope: PhantomData<&'scope ()>,
) -> silex_view::list::IndexedListView<'scope, IF, I, IS>
where
    IF: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = I> + 'scope,
    I: Clone + 'scope,
{
    let view_fn = Rc::new(move |item: I, index: usize| children.render(item, index));

    silex_view::list::IndexedListView {
        each,
        view_fn,
        _marker: std::marker::PhantomData,
    }
}
