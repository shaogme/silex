use silex_core::Runtime;
use silex_net::HttpClient;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let (id, _) = scope.signal(1_i32).unwrap();
        let builder = HttpClient::get(
            scope,
            "https://example.test",
            scope.error_handler(|_| {}).unwrap(),
        )
        .query("id", id);
        require_static(builder);
        let _task = scope.spawn_scoped(async move {}, scope.error_handler(|_| {}).unwrap());
    });
}
