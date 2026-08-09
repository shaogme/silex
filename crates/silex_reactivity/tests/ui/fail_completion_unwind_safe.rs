use std::{cell::RefCell, rc::Rc};

use silex_reactivity::Runtime;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let values = Rc::new(RefCell::new(Vec::<i32>::new()));
        let _sender = scope.completion_sender(move |value: i32| {
            values.borrow_mut().push(value);
            Ok::<(), ()>(())
        });
    });
}
