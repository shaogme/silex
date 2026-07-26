use silex_reactivity::*;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_memo_basic() {
    let s = signal::create(1);
    let m = memo::create(move |_old: Option<&i32>| {
        let val = signal::try_get::<i32>(s).unwrap_or(0);
        val * 2
    });

    assert_eq!(signal::try_get::<i32>(m), Ok(2));

    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(signal::try_get::<i32>(m), Ok(4));
}

#[test]
fn test_memo_efficiency() {
    let s = signal::create(1);
    let memo_runs = Rc::new(Cell::new(0));
    let runs_c = memo_runs.clone();

    let m = memo::create(move |_| {
        runs_c.set(runs_c.get() + 1);
        let val = signal::try_get::<i32>(s).unwrap_or(0);
        val % 2 // Only changes if parity changes
    });

    let effect_runs = Rc::new(Cell::new(0));
    let e_runs_c = effect_runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(m);
        e_runs_c.set(e_runs_c.get() + 1);
    });

    assert_eq!(memo_runs.get(), 1);
    assert_eq!(effect_runs.get(), 1);

    // Update to 2: parity changes
    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(memo_runs.get(), 2);
    assert_eq!(effect_runs.get(), 2);

    // Update to 4: parity does NOT change
    signal::update(s, |v: &mut i32| *v = 4);
    assert_eq!(memo_runs.get(), 3);
    assert_eq!(effect_runs.get(), 2); // Effect should NOT run because memo value didn't change
}

#[test]
fn test_derived_signal() {
    let s = signal::create(1);
    let d = memo::derived(Box::new(move || {
        let val = signal::try_get::<i32>(s).unwrap_or(0);
        val * 3
    }));

    assert_eq!(signal::try_get::<i32>(d), Ok(3));

    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(signal::try_get::<i32>(d), Ok(6));
}
