use silex_dom::prelude::*;
use silex_macros::styled;

styled! {
    pub Card<'scope><div>(children: AnyView<'scope>) {
        @apply flex flex-col gap-6 rounded-xl border bg-card py-6 text-card-foreground shadow-sm;
    }
}

styled! {
    pub CardHeader<'scope><div>(children: AnyView<'scope>) {
        @apply @container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-2 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6;
    }
}

styled! {
    pub CardTitle<'scope><div>(children: AnyView<'scope>) {
        @apply leading-none font-semibold;
    }
}

styled! {
    pub CardDescription<'scope><div>(children: AnyView<'scope>) {
        @apply text-sm text-muted-foreground;
    }
}

styled! {
    pub CardAction<'scope><div>(children: AnyView<'scope>) {
        @apply col-start-2 row-span-2 row-start-1 self-start justify-self-end;
    }
}

styled! {
    pub CardContent<'scope><div>(children: AnyView<'scope>) {
        @apply px-6;
    }
}

styled! {
    pub CardFooter<'scope><div>(children: AnyView<'scope>) {
        @apply flex items-center px-6 [.border-t]:pt-6;
    }
}
