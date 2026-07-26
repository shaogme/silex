use silex_reactivity::*;
use std::any::Any;

#[test]
fn test_stored_value() {
    let s = store::create(42i32);
    assert_eq!(store::try_with(s, |v: &i32| *v), Ok(42));

    let _ = store::try_update(s, |v: &mut i32| *v = 43);
    assert_eq!(store::try_with(s, |v: &i32| *v), Ok(43));

    // Stored value does NOT track dependencies
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let c = count.clone();
    effect::create(move || {
        let _ = store::try_with(s, |v: &i32| *v);
        c.set(c.get() + 1);
    });

    assert_eq!(count.get(), 1);
    let _ = store::try_update(s, |v: &mut i32| *v = 44);
    assert_eq!(count.get(), 1); // Should still be 1
}

#[test]
fn test_callback() {
    let result = std::rc::Rc::new(std::cell::Cell::new(0));
    let r = result.clone();

    let cb = callback::create(move |arg: Box<dyn Any>| {
        if let Some(val) = arg.downcast_ref::<i32>() {
            r.set(*val);
        }
    });

    let _ = callback::invoke(cb, Box::new(100i32));
    assert_eq!(result.get(), 100);
}

#[test]
fn test_node_ref() {
    let nr = node_ref::create::<i32>();
    assert_eq!(node_ref::get::<i32>(nr), None);

    let _ = node_ref::set(nr, 42i32);
    assert_eq!(node_ref::get::<i32>(nr), Some(42));
}
