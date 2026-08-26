use silex_core::{CloseError, ErrorHandler, HandlerLease, SilexResult};
use std::rc::Rc;

pub type MountEffect<'scope> = Box<dyn FnMut() -> SilexResult<()> + 'scope>;
pub type MountCleanup<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;
pub type MountErrorHandler<'scope> = ErrorHandler<'scope>;

pub(crate) type MountErrorLease<'scope> = HandlerLease<'scope>;
pub(crate) type CleanupReporter = Rc<dyn Fn(CloseError)>;
