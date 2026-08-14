use silex_core::{Scope, SilexResult};
use silex_dom::mounted::MountContext;

fn leak_scope<'scope>(
    ctx: &MountContext<'scope>,
    slot: &mut Option<Scope<'static>>,
) -> SilexResult<()> {
    *slot = Some(ctx.scope());
    Ok(())
}

fn main() {}
