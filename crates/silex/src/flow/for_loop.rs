use silex_core::ErrorHandlerToken;
use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead, RxReadRef};
use silex_macros::component;
use silex_view::elements::AnyView;
use silex_view::flow::{RenderOnlyKeyedListView, RowUpdater, StatefulKeyedListView};
use silex_view::mount::View;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

/// 类型擦除的 render-only keyed 列表行渲染器。
#[derive(Clone)]
pub struct ForRenderer<'scope, Item: 'scope> {
    render: Rc<dyn Fn(Item, usize) -> AnyView<'scope> + 'scope>,
}

impl<'scope, Item: 'scope> ForRenderer<'scope, Item> {
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

/// 类型擦除的 stateful keyed 列表行渲染器。
#[derive(Clone)]
pub struct ForStatefulRenderer<'scope, Item: 'scope> {
    render: Rc<dyn Fn(Item, usize, RowUpdater<'scope, Item>) -> AnyView<'scope> + 'scope>,
}

impl<'scope, Item: 'scope> ForStatefulRenderer<'scope, Item> {
    pub fn from_fn<F, V>(render: F) -> Self
    where
        F: Fn(Item, usize, RowUpdater<'scope, Item>) -> V + 'scope,
        V: View<'scope> + 'scope,
    {
        Self {
            render: Rc::new(move |item, index, updater| render(item, index, updater).into_any()),
        }
    }

    fn render(
        &self,
        item: Item,
        index: usize,
        updater: RowUpdater<'scope, Item>,
    ) -> AnyView<'scope> {
        (self.render)(item, index, updater)
    }
}

/// 标准 component 化的 render-only keyed `For` 组件。
///
/// `children` 的闭包参数在调用点直接推导为 `Item` 和 `usize`。
///
/// ```rust,ignore
/// For(list, |item| item.id)
///     .children(|item, idx| li(format!("{}: {}", idx, item.name)))
///     .error_handler(list_handler)
///     .build()
/// ```
#[component]
pub fn For<'scope, Ctx, ItemsFn, IS, Item, Key, KF>(
    #[ctx] ctx: Ctx,
    each: ItemsFn,
    key: KF,
    #[prop(render_fn(Item, usize))]
    #[chain]
    children: ForRenderer<'scope, Item>,
    #[prop(into)]
    #[chain(default)]
    row_error_handler: Option<ErrorHandlerToken<'scope>>,
    #[chain(default)] _scope: PhantomData<&'scope ()>,
) -> RenderOnlyKeyedListView<'scope, ItemsFn, IS, Item, Key>
where
    ItemsFn: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = Item> + Sized + 'scope,
    Item: Clone + 'scope,
    Key: Hash + Eq + Clone + 'scope,
    KF: Fn(&Item) -> Key + 'scope,
{
    let view_fn = Rc::new(move |item: Item, index: usize| children.render(item, index));

    RenderOnlyKeyedListView::new(each, Rc::new(key), view_fn, row_error_handler)
}

/// 需要显式绑定 `RowUpdater` 的 stateful keyed `For` 组件。
///
/// ```rust,ignore
/// ForStateful(list, |item| item.id)
///     .children(|item, idx, updater| {
///         let node = /* 创建并保存该行 DOM 节点 */;
///         updater.bind(move |next_item, next_idx| {
///             /* 增量更新 node */
///             Ok(())
///         });
///         node
///     })
///     .build()
/// ```
#[component]
pub fn ForStateful<'scope, Ctx, ItemsFn, IS, Item, Key, KF>(
    #[ctx] ctx: Ctx,
    each: ItemsFn,
    key: KF,
    #[prop(render_fn(Item, usize, RowUpdater<'scope, Item>))]
    #[chain]
    children: ForStatefulRenderer<'scope, Item>,
    #[prop(into)]
    #[chain(default)]
    row_error_handler: Option<ErrorHandlerToken<'scope>>,
    #[chain(default)] _scope: PhantomData<&'scope ()>,
) -> StatefulKeyedListView<'scope, ItemsFn, IS, Item, Key>
where
    ItemsFn: RxRead<Owned = IS> + RxReadRef<IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = Item> + Sized + 'scope,
    Item: Clone + 'scope,
    Key: Hash + Eq + Clone + 'scope,
    KF: Fn(&Item) -> Key + 'scope,
{
    let view_fn = Rc::new(
        move |item: Item, index: usize, updater: RowUpdater<'scope, Item>| {
            children.render(item, index, updater)
        },
    );

    StatefulKeyedListView::new(each, Rc::new(key), view_fn, row_error_handler)
}
