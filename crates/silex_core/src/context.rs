use crate::{ErrorReporter, OwnerAccess};

/// The explicit component ctx shared by Silex's core components.
#[derive(Clone, Copy)]
pub struct SilexContext<'owner> {
    owner: OwnerAccess<'owner>,
    error_reporter: ErrorReporter<'owner>,
}

impl<'owner> SilexContext<'owner> {
    /// Creates a ctx from the owner and its root error destination.
    pub fn new(owner: OwnerAccess<'owner>, error_reporter: ErrorReporter<'owner>) -> Self {
        Self {
            owner,
            error_reporter,
        }
    }

    /// Returns the runtime owner carried by this ctx.
    pub fn owner(self) -> OwnerAccess<'owner> {
        self.owner
    }

    /// Returns the error destination carried by this ctx.
    pub fn error_reporter(self) -> ErrorReporter<'owner> {
        self.error_reporter
    }

    /// Returns this ctx with only its error destination replaced.
    pub fn with_error_reporter(self, error_reporter: ErrorReporter<'owner>) -> Self {
        Self {
            owner: self.owner,
            error_reporter,
        }
    }
}

/// The capabilities required by a component ctx.
pub trait SilexContextProvider<'owner>: Clone + Copy + 'owner {
    fn owner(&self) -> OwnerAccess<'owner>;

    fn error_reporter(&self) -> ErrorReporter<'owner>;

    fn with_error_reporter(self, reporter: ErrorReporter<'owner>) -> Self;
}

impl<'owner> SilexContextProvider<'owner> for SilexContext<'owner> {
    fn owner(&self) -> OwnerAccess<'owner> {
        SilexContext::owner(*self)
    }

    fn error_reporter(&self) -> ErrorReporter<'owner> {
        SilexContext::error_reporter(*self)
    }

    fn with_error_reporter(self, reporter: ErrorReporter<'owner>) -> Self {
        SilexContext::with_error_reporter(self, reporter)
    }
}

#[cfg(test)]
mod tests {
    use super::SilexContext;
    use crate::{Runtime, SilexError};

    #[test]
    fn replacing_reporter_preserves_scope() {
        let mut runtime = Runtime::new();
        let root = runtime.owner().expect("runtime root should be created");

        root.with_access(|owner| {
            let first = owner
                .error_handler(|_: SilexError| {})
                .expect("first reporter should be registered");
            let second = owner
                .error_handler(|_: SilexError| {})
                .expect("second reporter should be registered");
            let ctx = SilexContext::new(owner, first.view());

            assert!(ctx.with_error_reporter(second.view()).owner() == owner);
        });
    }
}
