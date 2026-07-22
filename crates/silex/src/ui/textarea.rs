use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Textarea<textarea>() {
        @apply flex min-h-16 w-full rounded-md border border-solid border-slate-200 bg-transparent px-3 py-2 text-sm shadow-xs transition-colors placeholder:text-slate-500 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-950 focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 dark:border-slate-800 dark:placeholder:text-slate-400 dark:focus-visible:ring-slate-300;
    }
}
