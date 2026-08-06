use silex_core::reactivity::ReactiveSource;
use silex_core::traits::{ForErrorHandler, ForLoopSource, RxRead};
use silex_dom::prelude::*;
use silex_dom::view::RowUpdater;
use silex_macros::component;
use std::hash::Hash;
use std::marker::PhantomData;
use std::rc::Rc;

pub trait ForChildren<'scope, Item> {
    type View: View<'scope> + 'scope;

    fn render(&self, item: Item, index: usize, updater: RowUpdater<'scope, Item>) -> Self::View;
}

impl<'scope, Item, F, V> ForChildren<'scope, Item> for F
where
    F: Fn(Item, usize, RowUpdater<'scope, Item>) -> V,
    V: View<'scope> + 'scope,
{
    type View = V;

    fn render(&self, item: Item, index: usize, updater: RowUpdater<'scope, Item>) -> Self::View {
        self(item, index, updater)
    }
}

/// 标准 component 化的 For 组件。
///
/// 使用方式：
/// ```rust,ignore
/// For(list, |item| item.id)
///     .children(|item, idx, updater| li(format!("{}: {}", idx, item.name)))
///     .error(|err| log_error(err))
/// ```
#[component]
pub fn For<'scope, ItemsFn, IS, Item, Key, KF, MF>(
    each: ItemsFn,
    key: KF,
    #[prop(render)]
    #[chain]
    children: MF,
    #[prop(into)]
    #[chain(default = ForErrorHandler::default())]
    error: ForErrorHandler,
    #[chain(default)] _scope: PhantomData<&'scope ()>,
) -> silex_dom::view::list::KeyedLoopView<'scope, ItemsFn, IS, Item, Key>
where
    ItemsFn: RxRead<Value = IS> + ReactiveSource<'scope> + Clone + 'scope,
    IS: ForLoopSource<Item = Item> + Sized + 'scope,
    Item: Clone + 'scope,
    Key: Hash + Eq + Clone + 'scope,
    KF: Fn(&Item) -> Key + 'scope,
    MF: ForChildren<'scope, Item> + Clone + 'scope,
{
    let view_fn =
        Rc::new(move |item, index, updater| children.render(item, index, updater).into_any());

    silex_dom::view::list::KeyedLoopView {
        each,
        key_fn: Rc::new(key),
        view_fn,
        error,
        _marker: std::marker::PhantomData,
    }
}
