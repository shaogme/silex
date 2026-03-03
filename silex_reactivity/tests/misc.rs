use silex_reactivity::*;
use std::any::Any;

#[test]
fn test_stored_value() {
    let s = store_value(42i32);
    assert_eq!(try_with_stored_value(s, |v: &i32| *v), Some(42));

    try_update_stored_value(s, |v: &mut i32| *v = 43);
    assert_eq!(try_with_stored_value(s, |v: &i32| *v), Some(43));

    // Stored value does NOT track dependencies
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let c = count.clone();
    effect(move || {
        let _ = try_with_stored_value(s, |v: &i32| *v);
        c.set(c.get() + 1);
    });

    assert_eq!(count.get(), 1);
    try_update_stored_value(s, |v: &mut i32| *v = 44);
    assert_eq!(count.get(), 1); // Should still be 1
}

#[test]
fn test_callback() {
    let result = std::rc::Rc::new(std::cell::Cell::new(0));
    let r = result.clone();

    let cb = register_callback(move |arg: Box<dyn Any>| {
        if let Some(val) = arg.downcast_ref::<i32>() {
            r.set(*val);
        }
    });

    invoke_callback(cb, Box::new(100i32));
    assert_eq!(result.get(), 100);
}

#[test]
fn test_node_ref() {
    let nr = register_node_ref();
    assert_eq!(get_node_ref::<i32>(nr), None);

    set_node_ref(nr, 42i32);
    assert_eq!(get_node_ref::<i32>(nr), Some(42));
}
