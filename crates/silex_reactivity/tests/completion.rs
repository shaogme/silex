use silex_reactivity::{CompletionToken, Runtime};
use std::{cell::Cell, rc::Rc};

#[test]
fn completion_token_submits_while_scope_is_active() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let received = Rc::new(Cell::new(0));
        let received_by_callback = received.clone();
        let token = scope.completion(move |value: i32| {
            received_by_callback.set(value);
        });

        assert!(token.submit(7));
        assert!(token.submit(9));
        assert_eq!(received.get(), 9);
    });
}

#[test]
fn completion_token_is_invalid_after_child_scope_dispose() {
    let mut runtime = Runtime::new();
    let token = runtime.child(|scope| scope.child(|child| child.completion(|_: i32| {})));

    assert!(!token.submit(1));
}

#[test]
fn completion_token_is_invalid_after_run_returns() {
    let mut runtime = Runtime::new();
    let token: CompletionToken<i32> = runtime.child(|scope| scope.completion(|_: i32| {}));

    assert!(!token.submit(1));
}
