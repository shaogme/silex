use silex_core::{OwnerAccess, SilexResult};
use silex_dom::mounted::MountContext;

fn leak_scope<'scope>(
    ctx: &MountContext<'scope>,
    slot: &mut Option<OwnerAccess<'static>>,
) -> SilexResult<()> {
    *slot = Some(ctx.access());
    Ok(())
}

fn main() {}
