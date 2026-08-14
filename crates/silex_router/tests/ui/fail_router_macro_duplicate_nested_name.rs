use silex_router::macros::router;

router! {
    enum DuplicateNested {
        Users {
            prefix: "/users";
            layout: |_context, outlet| outlet;
            children: { List => "/" }
        },
        Users {
            prefix: "/accounts";
            layout: |_context, outlet| outlet;
            children: { List => "/" }
        },
    }
}

fn main() {}
