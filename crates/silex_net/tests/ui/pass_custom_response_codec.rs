use silex_net::{HttpClient, HttpMethod, NetError, ResponseCodec};
#[cfg(feature = "persist")]
use silex_net::{CacheCodec, TextCodec};

#[derive(Clone)]
struct Value;

#[derive(Clone)]
struct Codec;

impl ResponseCodec<Value> for Codec {
    fn decode(&self, _raw: &str) -> Result<Value, NetError> {
        Ok(Value)
    }
}

fn build(scope: silex_core::OwnerAccess<'_>) {
    let handler = scope.error_handler(|_| {}).unwrap();
    let send_builder = HttpClient::builder_with_codec(
        scope,
        HttpMethod::Get,
        "https://example.test",
        Codec,
        handler,
    );
    let _send = send_builder.send();

    let resource_builder = HttpClient::builder_with_codec(
        scope,
        HttpMethod::Get,
        "https://example.test",
        Codec,
        scope.error_handler(|_| {}).unwrap(),
    );
    let _resource = resource_builder.into_resource(None);

    let mutation_builder = HttpClient::builder_with_codec(
        scope,
        HttpMethod::Post,
        "https://example.test",
        Codec,
        scope.error_handler(|_| {}).unwrap(),
    );
    let _mutation = mutation_builder.as_mutation();
}

fn main() {
    let _ = build as fn(silex_core::OwnerAccess<'_>);
    #[cfg(feature = "persist")]
    require_cache_codec::<String, TextCodec>();
}

#[cfg(feature = "persist")]
fn require_cache_codec<T, C>()
where
    C: CacheCodec<T>,
{
}
