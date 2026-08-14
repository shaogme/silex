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
            layout: |_ctx| "invalid";
        },
    }
}

fn main() {}
