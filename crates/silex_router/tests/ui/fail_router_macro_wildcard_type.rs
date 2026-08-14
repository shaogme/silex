use silex_router::macros::router;

router! {
    enum WrongWildcard {
        Files { rest: String } => "/files/*rest",
    }
}

fn main() {}
