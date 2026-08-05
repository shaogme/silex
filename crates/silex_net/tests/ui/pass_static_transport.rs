use silex_net::{HttpMethod, RequestBody, RequestSpec, Transport};

fn main() {
    let transport = silex_net::BrowserTransport;
    let spec = RequestSpec {
        method: HttpMethod::Post,
        url: "https://example.test".to_string(),
        headers: Vec::new(),
        timeout: None,
        body: RequestBody::Text("body".to_string()),
    };
    let _future = transport.send(spec);
}
