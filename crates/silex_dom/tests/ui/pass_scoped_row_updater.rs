use silex_core::Runtime;
use silex_dom::view::{AnyView, RowUpdater, StatefulKeyedListView};
use std::{marker::PhantomData, rc::Rc};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let items = owner.signal(vec![1i32]).expect("signal");
            let view = StatefulKeyedListView {
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
        .expect("transient owner should initialize");
}
