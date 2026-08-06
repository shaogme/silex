use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Skeleton<div>() {
        @apply animate-pulse rounded-md bg-slate-100 dark:bg-slate-800;
    }
}
