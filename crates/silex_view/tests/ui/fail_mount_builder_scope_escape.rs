use silex_view::MountBuilderContext;

fn escape<'scope>(context: &MountBuilderContext<'scope>) -> &'scope MountBuilderContext<'scope> {
    context
}

fn main() {}
