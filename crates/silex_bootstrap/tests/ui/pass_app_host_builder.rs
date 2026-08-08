use silex_bootstrap::AppHost;
use silex_core::{Runtime, SilexResult};
use silex_dom::MountContext;

#[allow(dead_code)]
fn call_mount<F>(
    host: &mut AppHost,
    runtime: Runtime,
    builder: F,
) -> Result<(), silex_bootstrap::AppHostError>
where
    F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
{
    host.mount(runtime, builder)
}

fn main() {}
