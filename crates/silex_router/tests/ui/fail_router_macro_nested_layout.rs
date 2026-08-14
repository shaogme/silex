use silex_router::macros::router;

router! {
    enum UsersRoute {
        List => "/",
    }
}

router! {
    enum InvalidLayout {
        Users(UsersRoute) {
            prefix: "/users";
            layout: |_context| "invalid";
        },
    }
}

fn main() {}
