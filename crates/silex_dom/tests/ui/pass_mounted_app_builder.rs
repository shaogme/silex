use silex_core::{SilexError, SilexResult};
use silex_dom::mounted::MountContext;

fn builder<'scope>(context: &MountContext<'scope>) -> SilexResult<()> {
    let _scope = context.scope();
    let _parent = context.parent();
    let _handler = context.scope().error_handler(|_: SilexError| {})?;
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
