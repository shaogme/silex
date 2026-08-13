use silex_core::Runtime;
use silex_dom::view::{AnyView, KeyedListView, RowUpdater};
use std::{marker::PhantomData, rc::Rc};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (items, _) = scope.signal(vec![1i32]).expect("signal");
            let view = KeyedListView {
                each: items,
                key_fn: Rc::new(|item: &i32| *item),
                view_fn: Rc::new(|item: i32, index, updater: RowUpdater<'_, i32>| {
                    assert!(updater.bind(|_, _| {}));
                    AnyView::new(format!("{item}:{index}"))
                }),
                error_handler: None,
                _marker: PhantomData::<(Vec<i32>, i32)>,
            };
            let _ = view;
        })
        .expect("child scope should initialize");
}
