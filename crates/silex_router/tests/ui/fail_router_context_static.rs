use silex_core::Runtime;
use silex_router::{RouterContext, RouterContextProps};

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (path, set_path) = scope
            .signal(String::from("/"))
            .expect("path signal should be created");
        let (search, set_search) = scope
            .signal(String::new())
            .expect("search signal should be created");
        let context = RouterContext::new(
            scope,
            RouterContextProps {
                base_path: String::from("/"),
                path,
                search,
                set_path,
                set_search,
            },
            scope
                .error_handler(|_| {})
                .expect("error handler should be registered"),
        )
        .expect("router context should be created");
        require_static(context);
    });
}
