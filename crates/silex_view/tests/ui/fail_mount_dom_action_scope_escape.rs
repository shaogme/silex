use silex_view::MountDomAction;

fn escape<'scope>(action: MountDomAction<'scope>) -> MountDomAction<'static> {
    action
}

fn main() {}
