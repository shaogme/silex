use silex_core::Runtime;
use silex_net::HttpClient;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(1_i32);
        let _builder = HttpClient::get(
            scope,
            "https://example.test",
            scope.error_handler(|_| {}),
        )
            .query("value", move || value.get());
    });
}
