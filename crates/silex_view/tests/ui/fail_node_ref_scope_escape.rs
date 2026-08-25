use silex_dom::lifecycle::node_ref::NodeRef;

fn escape<'scope>(reference: NodeRef<'scope>) -> NodeRef<'static> {
    reference
}

fn main() {}
