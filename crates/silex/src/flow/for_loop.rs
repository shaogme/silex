use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForLoopSource, RxRead};
use silex_core::{ErrorHandler, ErrorReporter, SilexError};
use silex_dom::prelude::*;
use silex_dom::view::{AnyView, KeyedListView, RowUpdater};
use silex_macros::component;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

/// 类型擦除的 keyed 列表行渲染器。
#[derive(Clone)]
pub struct ForRenderer<'scope, Item: 'scope> {
    render: Rc<dyn Fn(Item, usize, RowUpdater<'scope, Item>) -> AnyView<'scope> + 'scope>,
}

impl<'scope, Item: 'scope> ForRenderer<'scope, Item> {
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

/// 标准 component 化的 keyed `For` 组件。
///
/// `children` 的闭包参数在调用点直接推导为 `Item`、`usize` 和 `RowUpdater`。
///
/// ```rust,ignore
/// For(list, |item| item.id)
///     .children(|item, idx, updater| li(format!("{}: {}", idx, item.name)))
///     .error_handler(list_handler)
///     .build()
/// ```
#[component]
pub fn For<'scope, ItemsFn, IS, Item, Key, KF>(
    each: ItemsFn,
    key: KF,
    #[prop(render_fn(Item, usize, RowUpdater<'scope, Item>))]
    #[chain]
    children: ForRenderer<'scope, Item>,
    #[prop(into)]
    #[chain(default)]
    row_error_handler: Option<ErrorHandler<'scope, SilexError>>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[chain(default)] _scope: PhantomData<&'scope ()>,
) -> KeyedListView<'scope, ItemsFn, IS, Item, Key>
where
    ItemsFn: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
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

    KeyedListView {
        each,
        key_fn: Rc::new(key),
        view_fn,
        error_handler: row_error_handler.or(Some(error_handler)),
        _marker: std::marker::PhantomData,
    }
}
