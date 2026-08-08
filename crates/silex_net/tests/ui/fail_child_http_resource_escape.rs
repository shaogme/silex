use silex_core::{Resource, Runtime};
use silex_net::{HttpClient, NetError};

fn escape(runtime: &mut Runtime) -> Resource<'static, std::string::String, NetError> {
    runtime.child(|scope| {
        HttpClient::get(scope, "https://example.test", scope.error_handler(|_| {}))
            .into_resource(None)
    })
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = escape(&mut runtime);
}
