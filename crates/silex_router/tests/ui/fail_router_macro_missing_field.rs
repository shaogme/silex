use silex_router::macros::router;

router! {
    enum MissingField {
        User => "/users/:id",
    }
}

fn main() {}
