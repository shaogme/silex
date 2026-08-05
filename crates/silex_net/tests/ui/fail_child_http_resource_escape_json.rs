use silex_core::{Resource, Runtime};
use silex_net::{HttpClient, NetError};

fn escape(runtime: &mut Runtime) -> Resource<'static, String, NetError> {
    runtime.child(|scope| HttpClient::get(scope, "https://example.test").into_resource(None))
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = escape(&mut runtime);
}
