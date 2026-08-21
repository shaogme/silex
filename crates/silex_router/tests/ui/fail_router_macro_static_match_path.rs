use silex_router::macros::router;

router! {
    enum AppRoute {
        Home => "/",
    }
}

fn main() {
    let _ = AppRoute::match_path("/");
}
