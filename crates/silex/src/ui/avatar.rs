use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Avatar<div>(children: AnyView) {
        @apply relative flex size-10 shrink-0 overflow-hidden rounded-full select-none;
    }
}

styled! {
    pub AvatarImage<img>() {
        @apply aspect-square size-full object-cover;
    }
}

styled! {
    pub AvatarFallback<div>(children: AnyView) {
        @apply flex size-full items-center justify-center rounded-full bg-slate-100 dark:bg-slate-800 text-sm font-medium text-slate-500 dark:text-slate-400;
    }
}
