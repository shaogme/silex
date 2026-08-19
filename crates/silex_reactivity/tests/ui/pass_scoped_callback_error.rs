use silex_reactivity::{Callback, CallbackInvokeError, Runtime};
use std::{cell::Cell, rc::Rc};

struct Input<'a>(&'a str);

struct UserError<'a> {
    value: &'a str,
    calls: Rc<Cell<usize>>,
}

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
        let text = String::from("scoped callback");
        let calls = Rc::new(Cell::new(0));
        let calls_in_callback = calls.clone();
        let callback: Callback<'_, Input<'_>, UserError<'_>> = scope
            .callback(move |input: Input<'_>| {
                calls_in_callback.set(calls_in_callback.get() + 1);
                Err(UserError {
                    value: input.0,
                    calls: calls_in_callback.clone(),
                })
            })
            .expect("callback should initialize");

        let error = callback
            .invoke(Input(text.as_str()))
            .expect_err("callback should return its borrowed user error");
        match error {
            CallbackInvokeError::User(error) => {
                assert_eq!(error.value, "scoped callback");
                assert_eq!(error.calls.get(), 1);
            }
            CallbackInvokeError::Runtime(_) => panic!("expected a user error"),
            CallbackInvokeError::Handler(_) => panic!("expected a user error"),
        }
        })
        .expect("child scope should complete");
}
