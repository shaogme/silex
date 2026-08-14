use silex_core::{SilexError, SilexResult};
use silex_dom::mounted::MountContext;

fn builder<'scope>(ctx: &MountContext<'scope>) -> SilexResult<()> {
    let _scope = ctx.scope();
    let _parent = ctx.parent();
    let _handler = ctx.scope().error_handler(|_: SilexError| {})?;
    Ok(())
}

fn accepts_builder<F>(_: F)
where
    F: for<'scope> FnOnce(&MountContext<'scope>) -> SilexResult<()>,
{
}

fn main() {
    accepts_builder(builder);
}
