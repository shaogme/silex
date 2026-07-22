use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Card<div>(children: AnyView) {
        @apply flex flex-col gap-6 rounded-xl border bg-card py-6 text-card-foreground shadow-sm;
    }
}

styled! {
    pub CardHeader<div>(children: AnyView) {
        @apply @container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-2 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6;
    }
}

styled! {
    pub CardTitle<div>(children: AnyView) {
        @apply leading-none font-semibold;
    }
}

styled! {
    pub CardDescription<div>(children: AnyView) {
        @apply text-sm text-muted-foreground;
    }
}

styled! {
    pub CardAction<div>(children: AnyView) {
        @apply col-start-2 row-span-2 row-start-1 self-start justify-self-end;
    }
}

styled! {
    pub CardContent<div>(children: AnyView) {
        @apply px-6;
    }
}

styled! {
    pub CardFooter<div>(children: AnyView) {
        @apply flex items-center px-6 [.border-t]:pt-6;
    }
}
