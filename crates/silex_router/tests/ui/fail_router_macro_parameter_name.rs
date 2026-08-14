use silex_router::macros::router;

router! {
    enum WrongName {
        User { user_id: u32 } => "/users/:id",
    }
}

fn main() {}
