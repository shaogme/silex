use silex_bootstrap::AppHost;
use silex_core::{OwnerAccess, Runtime, SilexResult};
use silex_dom::MountContext;

#[allow(dead_code)]
fn call_mount(
    host: &mut AppHost,
    runtime: Runtime,
    slot: &mut Option<OwnerAccess<'static>>,
) {
    let _ = host.mount(runtime, |ctx| {
        *slot = Some(ctx.access());
        Ok(())
    });
}

#[allow(dead_code)]
fn builder<'scope>(
    ctx: &MountContext<'scope>,
    slot: &mut Option<OwnerAccess<'static>>,
) -> SilexResult<()> {
    *slot = Some(ctx.access());
    Ok(())
}

fn main() {}
