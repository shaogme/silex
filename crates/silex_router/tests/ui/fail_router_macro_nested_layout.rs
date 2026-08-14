use silex_router::macros::router;

router! {
    enum InvalidLayout {
        Users {
            prefix: "/users";
            layout: |_context| "invalid";
            children: { List => "/" }
        },
    }
}

fn main() {}
