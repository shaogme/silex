use silex_core::{Scope, SilexResult};
use silex_dom::mounted::MountContext;

fn leak_scope<'scope>(
    context: &MountContext<'scope>,
    slot: &mut Option<Scope<'static>>,
) -> SilexResult<()> {
    *slot = Some(context.scope());
    Ok(())
}

fn main() {}
