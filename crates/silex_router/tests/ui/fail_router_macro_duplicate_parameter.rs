use silex_router::macros::router;

router! {
    enum DuplicateParameter {
        User { id: u32 } => "/users/:id/:id",
    }
}

fn main() {}
