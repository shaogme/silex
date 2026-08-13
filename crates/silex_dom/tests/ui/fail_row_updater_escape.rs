use silex_core::Runtime;
use silex_dom::view::{AnyView, RowUpdater, StatefulKeyedListView};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

fn escape<'scope>(updater: RowUpdater<'scope, i32>) -> RowUpdater<'static, i32> {
    updater
}

fn main() {
    let mut runtime = Runtime::new();
    let saved = runtime.child(|scope| {
        let (items, _) = scope.signal(vec![1i32]).expect("signal");
        let saved = Rc::new(RefCell::new(None));
        let saved_for_factory = saved.clone();
        let view = StatefulKeyedListView {
            each: items,
            key_fn: Rc::new(|item: &i32| *item),
            view_fn: Rc::new(move |item: i32, index, updater: RowUpdater<'_, i32>| {
                *saved_for_factory.borrow_mut() = Some(updater);
                AnyView::new(format!("{item}:{index}"))
            }),
            error_handler: None,
            _marker: PhantomData::<(Vec<i32>, i32)>,
        };
        let _ = view;
        saved
    });
    let _ = saved;
}
