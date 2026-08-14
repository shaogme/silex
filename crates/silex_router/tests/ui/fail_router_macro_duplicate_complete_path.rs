use silex_router::macros::router;

router! {
    enum DuplicateComplete {
        Users {
            prefix: "/users";
            layout: |_context, outlet| outlet;
            children: {
                Detail { id: u32 } => "/:id",
            }
        },
        Direct { id: u32 } => "/users/:id",
    }
}

fn main() {}
