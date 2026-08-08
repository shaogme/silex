use silex_bootstrap::AppHost;
use silex_core::{Runtime, Scope, SilexResult};
use silex_dom::MountContext;

#[allow(dead_code)]
fn call_mount(
    host: &mut AppHost,
    runtime: Runtime,
    slot: &mut Option<Scope<'static>>,
) {
    let _ = host.mount(runtime, |context| {
        *slot = Some(context.scope());
        Ok(())
    });
}

#[allow(dead_code)]
fn builder<'scope>(
    context: &MountContext<'scope>,
    slot: &mut Option<Scope<'static>>,
) -> SilexResult<()> {
    *slot = Some(context.scope());
    Ok(())
}

fn main() {}
