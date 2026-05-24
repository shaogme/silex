use silex_core::reactivity::ReadSignal;
use silex_core::traits::{ForErrorHandler, ForLoopSource, RxRead};
use silex_dom::prelude::*;
use silex_macros::component;
use std::hash::Hash;
use std::rc::Rc;

/// 标准 component 化的 For 组件。
///
/// 使用方式：
/// ```rust,ignore
/// For(list, |item| item.id)
///     .children(|item, idx| li(format!("{}: {}", idx.get(), item.name)))
///     .error(|err| log_error(err))
/// ```
#[component]
pub fn For<ItemsFn, IS, Item, Key, MF, V>(
    each: ItemsFn,
    key: fn(&Item) -> Key,
    #[prop(render)]
    #[chain]
    children: MF,
    #[prop(into)]
    #[chain(default = ForErrorHandler::default())]
    error: ForErrorHandler,
) -> impl View
where
    ItemsFn: RxRead<Value = IS> + Clone + 'static,
    IS: ForLoopSource<Item = Item> + Sized + 'static,
    Item: Clone + 'static,
    Key: Hash + Eq + Clone + 'static,
    MF: Fn(ReadSignal<Item>, ReadSignal<usize>) -> V + Clone + 'static,
    V: View + 'static,
{
    let view_fn = Rc::new(move |item, index| children(item, index).into_any());

    silex_dom::view::list::KeyedLoopView {
        each,
        key_fn: key,
        view_fn,
        error,
        _marker: std::marker::PhantomData,
    }
}
