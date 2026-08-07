use silex_core::Runtime;
use silex_net::HttpClient;

fn require_static<T: 'static>(_: T) {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (id, _) = scope.signal(1_i32);
        let builder = HttpClient::get(
            scope,
            "https://example.test",
            silex_core::ErrorReporter::new(|_| {}),
        )
        .query("id", id);
        require_static(builder);
        scope.spawn_scoped(async move {}, silex_core::ErrorReporter::new(|_| {}));
    });
}
