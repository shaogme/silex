use silex_reactivity::ErrorHandlerToken;

fn main() {
    let _: ErrorHandlerToken<'static, ()> = ErrorHandlerToken::new(|_: ()| {});
}
