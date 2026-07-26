use silex_reactivity::*;

#[test]
fn test_signal_basic() {
    let s = signal::create(1);
    assert_eq!(signal::try_get::<i32>(s), Ok(1));

    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(signal::try_get::<i32>(s), Ok(2));
}

#[test]
fn test_signal_untracked() {
    let s = signal::create(1);
    assert_eq!(signal::try_get_untracked::<i32>(s), Ok(1));
}

#[test]
fn test_signal_validity() {
    let s = signal::create(1);
    assert!(s.is_alive());
    scope::dispose(s);
    // After dispose, it should be invalid
    // Note: depending on the implementation of dispose, it might be removed from reactive map
    assert!(!s.is_alive());
}

#[test]
fn test_signal_with() {
    let s = signal::create(10);
    let val = signal::try_with(s, |v: &i32| *v * 2);
    assert_eq!(val, Ok(20));
}

#[test]
fn test_update_signal_silent() {
    let s = signal::create(10);
    let updated = signal::try_update_silent(s, |v: &mut i32| {
        *v = 20;
        *v * 2
    });
    assert_eq!(updated, Ok(40));
    assert_eq!(signal::try_get_untracked::<i32>(s), Ok(20));
}
