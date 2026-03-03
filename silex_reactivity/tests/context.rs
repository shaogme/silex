use silex_reactivity::*;

#[test]
fn test_context_basic() {
    create_scope(|| {
        provide_context(42i32);
        provide_context("hello".to_string());

        assert_eq!(use_context::<i32>(), Some(42));
        assert_eq!(use_context::<String>(), Some("hello".to_string()));
    });
}

#[test]
fn test_context_inheritance() {
    create_scope(|| {
        provide_context(42i32);

        create_scope(|| {
            assert_eq!(use_context::<i32>(), Some(42));
            provide_context(100i32); // Override
            assert_eq!(use_context::<i32>(), Some(100));
        });

        // Parent still has old value
        assert_eq!(use_context::<i32>(), Some(42));
    });
}

#[test]
fn test_context_missing() {
    create_scope(|| {
        assert_eq!(use_context::<f64>(), None);
    });
}
