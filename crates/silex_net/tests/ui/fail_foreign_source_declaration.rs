use silex_core::Runtime;
use silex_net::HttpClient;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let (value, _) = scope.signal(1_i32).unwrap();
        let _builder = HttpClient::get(
            scope,
            "https://example.test",
            scope.error_handler(|_| {}).unwrap(),
        )
            .query("value", move || value.get().unwrap());
    });
}
