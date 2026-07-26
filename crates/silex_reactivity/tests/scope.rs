use silex_reactivity::*;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_scope_dispose() {
    let s = signal::create(1);
    let effect_runs = Rc::new(Cell::new(0));
    let runs_c = effect_runs.clone();

    let scope = scope::create(move || {
        effect::create(move || {
            let _ = signal::try_get::<i32>(s);
            runs_c.set(runs_c.get() + 1);
        });
    });

    assert_eq!(effect_runs.get(), 1);
    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(effect_runs.get(), 2);

    scope::dispose(scope);

    signal::update(s, |v: &mut i32| *v = 3);
    assert_eq!(effect_runs.get(), 2); // Effect shouldn't run after scope is disposed
}

#[test]
fn test_nested_scope_dispose() {
    let s = signal::create(1);
    let run1 = Rc::new(Cell::new(0));
    let run2 = Rc::new(Cell::new(0));

    let c1 = run1.clone();
    let c2 = run2.clone();

    let outer = scope::create(move || {
        effect::create(move || {
            let _ = signal::try_get::<i32>(s);
            c1.set(c1.get() + 1);
        });

        scope::create(move || {
            effect::create(move || {
                let _ = signal::try_get::<i32>(s);
                c2.set(c2.get() + 1);
            });
        });
    });

    assert_eq!(run1.get(), 1);
    assert_eq!(run2.get(), 1);

    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(run1.get(), 2);
    assert_eq!(run2.get(), 2);

    scope::dispose(outer);

    signal::update(s, |v: &mut i32| *v = 3);
    assert_eq!(run1.get(), 2);
    assert_eq!(run2.get(), 2);
}

#[test]
fn test_on_cleanup_recursive() {
    let cleanup_order = Rc::new(std::cell::RefCell::new(Vec::new()));

    let o1 = cleanup_order.clone();
    let o2 = cleanup_order.clone();
    let o3 = cleanup_order.clone();

    let scope = scope::create(move || {
        scope::on_cleanup(move || o1.borrow_mut().push(1));

        scope::create(move || {
            scope::on_cleanup(move || o2.borrow_mut().push(2));
            scope::on_cleanup(move || o3.borrow_mut().push(3));
        });
    });

    scope::dispose(scope);

    // Child scope is cleaned up before parent.
    // Within a scope, cleanups are currently FIFO (order of registration).
    let result = cleanup_order.borrow().clone();
    assert_eq!(result, vec![2, 3, 1]);
}
