use std::{cell::RefCell, rc::Rc};

use silex_reactivity::{Runtime, unwind_safe};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let values = Rc::new(RefCell::new(Vec::<i32>::new()));
        let values_for_callback = values.clone();
        let sender = scope.completion_sender(unwind_safe(move |value: i32| {
            values_for_callback.borrow_mut().push(value);
            Ok::<(), ()>(())
        }));
        assert!(sender.submit(1).expect("completion submit"));
    });
}
