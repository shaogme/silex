use silex_view::mount::MountDomAction;

fn escape<'scope>(action: MountDomAction<'scope>) -> MountDomAction<'static> {
    action
}

fn main() {}
