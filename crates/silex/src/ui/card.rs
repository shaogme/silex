use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Card<div>(children: AnyView) {
        @apply flex flex-col gap-6 rounded-xl border bg-card py-6 text-card-foreground shadow-sm;
    }
}

styled! {
    pub CardHeader<div>(children: AnyView) {
        @apply flex flex-col gap-1.5 px-6;
    }
}

styled! {
    pub CardTitle<h3>(children: AnyView) {
        @apply text-lg font-semibold leading-none tracking-tight;
    }
}

styled! {
    pub CardDescription<p>(children: AnyView) {
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
        @apply flex items-center px-6;
    }
}
