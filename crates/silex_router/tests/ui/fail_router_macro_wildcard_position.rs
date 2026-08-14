use silex_router::macros::router;

router! {
    enum WildcardPosition {
        Files { rest: silex_router::PathTail } => "/files/*rest/more",
    }
}

fn main() {}
