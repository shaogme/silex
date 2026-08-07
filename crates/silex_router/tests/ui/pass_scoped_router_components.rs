use silex_core::Runtime;
use silex_router::{Link, Router};
use silex_router::dom::attribute::GlobalEventAttributes;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (text, _) = scope.signal(String::from("scoped"));
        let _link = Link("/")
            .children(text)
            .active_class("active")
            .on_click(|_| Ok(()));
        let _router = Router(scope).base("/app");
    });
}
