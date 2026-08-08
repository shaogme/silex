use silex_reactivity::ErrorHandler;

fn main() {
    let _: ErrorHandler<'static, ()> = ErrorHandler::new(|_: ()| {});
}
