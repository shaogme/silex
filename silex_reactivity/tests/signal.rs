use silex_reactivity::*;

#[test]
fn test_signal_basic() {
    let s = signal(1);
    assert_eq!(try_get_signal::<i32>(s), Some(1));

    update_signal(s, |v: &mut i32| *v = 2);
    assert_eq!(try_get_signal::<i32>(s), Some(2));
}

#[test]
fn test_signal_untracked() {
    let s = signal(1);
    assert_eq!(try_get_signal_untracked::<i32>(s), Some(1));
}

#[test]
fn test_signal_validity() {
    let s = signal(1);
    assert!(is_signal_valid(s));
    dispose(s);
    // After dispose, it should be invalid
    // Note: depending on the implementation of dispose, it might be removed from reactive map
    assert!(!is_signal_valid(s));
}

#[test]
fn test_signal_with() {
    let s = signal(10);
    let val = try_with_signal(s, |v: &i32| *v * 2);
    assert_eq!(val, Some(20));
}

#[test]
fn test_update_signal_silent() {
    let s = signal(10);
    let updated = try_update_signal_silent(s, |v: &mut i32| {
        *v = 20;
        *v * 2
    });
    assert_eq!(updated, Some(40));
    assert_eq!(try_get_signal_untracked::<i32>(s), Some(20));
}
