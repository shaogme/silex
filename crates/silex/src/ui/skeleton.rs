use silex_core::{ErrorReporter, Scope};
use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Skeleton<'scope><div>(
        scope: Scope<'scope>,
        #[chain] error_handler: ErrorReporter<'scope>,
    ) {
        @apply animate-pulse rounded-md bg-slate-100 dark:bg-slate-800;
    }
}
