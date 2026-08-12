use silex_reactivity::{ErrorHandler, Runtime};

fn main() {
    let mut runtime = Runtime::new();
    let handler = runtime
        .child(|scope| {
            let local = String::from("scoped");
            let value = &local;
            scope
                .error_handler(move |_: ()| {
                    assert_eq!(value.as_str(), "scoped");
                })
                .expect("error handler registration should succeed")
        })
        .expect("child scope should succeed");
    let _: ErrorHandler<'static, ()> = handler;
}
