use crate::lifecycle::MountOwnerToken;
use silex_core::CloseError;
use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

pub(crate) fn close_scope<'scope>(
    scope: MountOwnerToken<'scope>,
    reporter: &MountOwnerToken<'scope>,
) -> Result<(), CloseError> {
    match catch_unwind(AssertUnwindSafe(|| scope.close())) {
        Ok(result) => result,
        Err(panic) => {
            let error = CloseError::from_panic(panic);
            reporter.report_close_error(error.clone());
            Err(error)
        }
    }
}

pub(crate) fn panic_message(prefix: &str, panic: Box<dyn Any + Send>) -> String {
    if let Some(value) = panic.downcast_ref::<&str>() {
        format!("{prefix}: {value}")
    } else if let Some(value) = panic.downcast_ref::<String>() {
        format!("{prefix}: {value}")
    } else {
        format!("{prefix}: unknown panic")
    }
}
