use silex_router::macros::router;

router! {
    enum Duplicate {
        First { id: u32 } => "/users/:id",
        Second { name: u32 } => "/users/:name",
    }
}

fn main() {}
