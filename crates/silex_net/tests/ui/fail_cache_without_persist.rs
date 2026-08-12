use silex_core::Runtime;
use silex_net::{CachePolicy, HttpClient};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _builder = HttpClient::get(
            scope,
            "https://example.test",
            scope.error_handler(|_| {}).unwrap(),
        )
        .cache_with_default(CachePolicy::CacheFirst, "default".to_string());
    });
}
