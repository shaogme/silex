use crate::{ErrorReporter, Scope};

/// The explicit component ctx shared by Silex's core components.
#[derive(Clone, Copy)]
pub struct SilexContext<'scope> {
    scope: Scope<'scope>,
    error_reporter: ErrorReporter<'scope>,
}

impl<'scope> SilexContext<'scope> {
    /// Creates a ctx from the scope and its root error destination.
    pub fn new(scope: Scope<'scope>, error_reporter: ErrorReporter<'scope>) -> Self {
        Self {
            scope,
            error_reporter,
        }
    }

    /// Returns the runtime scope carried by this ctx.
    pub fn scope(self) -> Scope<'scope> {
        self.scope
    }

    /// Returns the error destination carried by this ctx.
    pub fn error_reporter(self) -> ErrorReporter<'scope> {
        self.error_reporter
    }

    /// Returns this ctx with only its error destination replaced.
    pub fn with_error_reporter(self, error_reporter: ErrorReporter<'scope>) -> Self {
        Self {
            scope: self.scope,
            error_reporter,
        }
    }
}

/// The capabilities required by a component ctx.
pub trait SilexContextProvider<'scope>: Clone + Copy + 'scope {
    fn scope(&self) -> Scope<'scope>;

    fn error_reporter(&self) -> ErrorReporter<'scope>;

    fn with_error_reporter(self, reporter: ErrorReporter<'scope>) -> Self;
}

impl<'scope> SilexContextProvider<'scope> for SilexContext<'scope> {
    fn scope(&self) -> Scope<'scope> {
        SilexContext::scope(*self)
    }

    fn error_reporter(&self) -> ErrorReporter<'scope> {
        SilexContext::error_reporter(*self)
    }

    fn with_error_reporter(self, reporter: ErrorReporter<'scope>) -> Self {
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
        let root = runtime.run().expect("runtime root should be created");

        root.with_scope(|scope| {
            let first = scope
                .error_handler(|_: SilexError| {})
                .expect("first reporter should be registered");
            let second = scope
                .error_handler(|_: SilexError| {})
                .expect("second reporter should be registered");
            let ctx = SilexContext::new(scope, first.view());

            assert!(ctx.with_error_reporter(second.view()).scope() == scope);
        });
    }
}
