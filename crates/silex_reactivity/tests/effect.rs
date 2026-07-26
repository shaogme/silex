use silex_reactivity::*;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_effect_basic() {
    let s = signal::create(1);
    let count = Rc::new(Cell::new(0));
    let count_c = count.clone();

    effect::create(move || {
        let _val = signal::try_get::<i32>(s);
        count_c.set(count_c.get() + 1);
    });

    assert_eq!(count.get(), 1);

    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(count.get(), 2);

    signal::update(s, |v: &mut i32| *v = 3);
    assert_eq!(count.get(), 3);
}

#[test]
fn test_effect_multiple_signals() {
    let s1 = signal::create(1);
    let s2 = signal::create(10);
    let result = Rc::new(Cell::new(0));
    let result_c = result.clone();

    effect::create(move || {
        let v1 = signal::try_get::<i32>(s1).unwrap_or(0);
        let v2 = signal::try_get::<i32>(s2).unwrap_or(0);
        result_c.set(v1 + v2);
    });

    assert_eq!(result.get(), 11);

    signal::update(s1, |v: &mut i32| *v = 2);
    assert_eq!(result.get(), 12);

    signal::update(s2, |v: &mut i32| *v = 20);
    assert_eq!(result.get(), 22);
}

#[test]
fn test_effect_untrack() {
    let s1 = signal::create(1);
    let s2 = signal::create(10);
    let result = Rc::new(Cell::new(0));
    let result_c = result.clone();

    effect::create(move || {
        let v1 = signal::try_get::<i32>(s1).unwrap_or(0);
        let v2 = scope::untrack(|| signal::try_get::<i32>(s2).unwrap_or(0));
        result_c.set(v1 + v2);
    });

    assert_eq!(result.get(), 11);

    signal::update(s1, |v: &mut i32| *v = 2);
    assert_eq!(result.get(), 12);

    // Changing s2 should NOT trigger the effect
    signal::update(s2, |v: &mut i32| *v = 20);
    assert_eq!(result.get(), 12);

    // Changing s1 again should use the NEW s2 value
    signal::update(s1, |v: &mut i32| *v = 3);
    assert_eq!(result.get(), 23);
}

#[test]
fn test_effect_cleanup() {
    let s = signal::create(1);
    let cleaned_up = Rc::new(Cell::new(false));
    let cleaned_up_c = cleaned_up.clone();

    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        let c = cleaned_up_c.clone();
        scope::on_cleanup(move || {
            c.set(true);
        });
    });

    assert!(!cleaned_up.get());

    signal::update(s, |v: &mut i32| *v = 2);
    assert!(cleaned_up.get());
}

#[test]
fn test_batch() {
    let s = signal::create(1);
    let count = Rc::new(Cell::new(0));
    let count_c = count.clone();

    effect::create(move || {
        let _val = signal::try_get::<i32>(s);
        count_c.set(count_c.get() + 1);
    });

    assert_eq!(count.get(), 1);

    scope::batch(|| {
        signal::update(s, |v: &mut i32| *v = 2);
        signal::update(s, |v: &mut i32| *v = 3);
        signal::update(s, |v: &mut i32| *v = 4);
    });

    assert_eq!(count.get(), 2);
}
