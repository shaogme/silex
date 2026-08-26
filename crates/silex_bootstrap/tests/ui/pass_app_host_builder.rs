use silex_bootstrap::AppHost;
use silex_core::{Runtime, SilexResult};
use silex_view::app::MountBuilderContext;

#[allow(dead_code)]
fn call_mount<F>(
    host: &mut AppHost,
    runtime: Runtime,
    builder: F,
) -> SilexResult<()>
where
    F: for<'scope> FnOnce(&MountBuilderContext<'scope>) -> SilexResult<()>,
{
    host.mount(runtime, builder)
}

fn main() {}
