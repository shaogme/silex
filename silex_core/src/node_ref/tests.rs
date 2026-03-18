use super::NodeRef;
use silex_reactivity::create_scope;

#[test]
fn test_node_ref_new_and_get() {
    create_scope(|| {
        let node_ref = NodeRef::<String>::new();
        assert_eq!(node_ref.get(), None);
    });
}
