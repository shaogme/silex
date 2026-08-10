use silex_core::{SilexError, SilexResult, traits::ForLoopSource};

#[test]
fn fallible_list_source_exposes_successful_items() {
    let source: SilexResult<Vec<i32>> = Ok(vec![1, 2, 3]);

    assert_eq!(
        source.as_slice().expect("list should be available"),
        [1, 2, 3]
    );
}

#[test]
fn fallible_list_source_accepts_borrowed_items() {
    let item = String::from("borrowed item");
    let source: SilexResult<Vec<&str>> = Ok(vec![item.as_str()]);

    assert_eq!(
        source.as_slice().expect("list should be available"),
        ["borrowed item"]
    );
}

#[test]
fn fallible_list_source_preserves_errors() {
    let source: SilexResult<Vec<i32>> = Err(SilexError::Framework("list failed".to_string()));

    assert!(matches!(
        source.as_slice(),
        Err(SilexError::Framework(message)) if message == "list failed"
    ));
}
