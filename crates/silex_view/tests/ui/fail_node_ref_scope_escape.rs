use silex_dom::node_ref::NodeRef;

fn escape<'scope>(reference: NodeRef<'scope>) -> NodeRef<'static> {
    reference
}

fn main() {}
