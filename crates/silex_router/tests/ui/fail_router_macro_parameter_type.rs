use silex_router::macros::router;

router! {
    enum MissingType {
        User { id } => "/users/:id",
    }
}

fn main() {}
