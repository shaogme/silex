use silex_core::{RootWriteSignal, Runtime};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn high_level_root_survives_run_callback() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let setter = Rc::new(RefCell::new(None::<RootWriteSignal<i32>>));
    let setter_for_mount = setter.clone();
    let seen_for_mount = seen.clone();

    let mut root = runtime.run(move |scope| {
        let (value, set_value) = scope.signal(0i32);
        let seen = seen_for_mount.clone();
        scope.effect(move || seen.set(value.get()));
        *setter_for_mount.borrow_mut() = Some(set_value);
    });

    setter
        .borrow()
        .as_ref()
        .expect("root setter should be registered")
        .set(4);
    assert_eq!(seen.get(), 4);

    root.dispose().expect("root disposal should succeed");
    setter
        .borrow()
        .as_ref()
        .expect("root setter should remain invalid but safe")
        .set(5);
    assert_eq!(seen.get(), 4);
}
