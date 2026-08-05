use crate::NetError;
#[cfg(feature = "persist")]
use silex_core::Scope;
#[cfg(feature = "json")]
use std::marker::PhantomData;

pub trait ResponseCodec<T>: Clone + 'static {
    fn decode(&self, raw: &str) -> Result<T, NetError>;
}

#[cfg(feature = "persist")]
pub trait CacheCodec<T>: ResponseCodec<T> {
    fn build_cache<'scope>(
        scope: Scope<'scope>,
        key: String,
        default: T,
    ) -> silex_persist::Persistent<'scope, T>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCodec;

impl ResponseCodec<String> for TextCodec {
    fn decode(&self, raw: &str) -> Result<String, NetError> {
        Ok(raw.to_string())
    }
}

#[cfg(feature = "persist")]
impl silex_persist::PersistCodec<String> for TextCodec {
    fn encode(&self, value: &String) -> Result<String, String> {
        Ok(value.clone())
    }

    fn decode(&self, raw: &str) -> Result<String, String> {
        Ok(raw.to_string())
    }
}

#[cfg(feature = "persist")]
impl CacheCodec<String> for TextCodec {
    fn build_cache<'scope>(
        scope: Scope<'scope>,
        key: String,
        default: String,
    ) -> silex_persist::Persistent<'scope, String> {
        silex_persist::Persistent::builder(scope, key)
            .local()
            .string()
            .write_default(silex_persist::WriteDefault::Never)
            .default(default)
            .build()
    }
}

#[cfg(feature = "json")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetJsonCodec<T>(pub PhantomData<T>);

#[cfg(feature = "json")]
impl<T> NetJsonCodec<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[cfg(feature = "json")]
impl<T> ResponseCodec<T> for NetJsonCodec<T>
where
    T: serde::de::DeserializeOwned + Clone + 'static,
{
    fn decode(&self, raw: &str) -> Result<T, NetError> {
        serde_json::from_str(raw).map_err(|err| NetError::DecodeError(err.to_string()))
    }
}

#[cfg(all(feature = "json", feature = "persist"))]
impl<T> silex_persist::PersistCodec<T> for NetJsonCodec<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static,
{
    fn encode(&self, value: &T) -> Result<String, String> {
        serde_json::to_string(value).map_err(|err| err.to_string())
    }

    fn decode(&self, raw: &str) -> Result<T, String> {
        serde_json::from_str(raw).map_err(|err| err.to_string())
    }
}

#[cfg(all(feature = "json", feature = "persist"))]
impl<T> CacheCodec<T> for NetJsonCodec<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + PartialEq + 'static,
{
    fn build_cache<'scope>(
        scope: Scope<'scope>,
        key: String,
        default: T,
    ) -> silex_persist::Persistent<'scope, T> {
        silex_persist::Persistent::builder(scope, key)
            .local()
            .json::<T>()
            .write_default(silex_persist::WriteDefault::Never)
            .default(default)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::{ResponseCodec, TextCodec};

    #[test]
    fn text_codec_preserves_response_body() {
        assert_eq!(TextCodec.decode("hello").unwrap(), "hello");
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_codec_reports_decode_errors() {
        use super::NetJsonCodec;

        let codec = NetJsonCodec::<u32>::new();
        assert_eq!(codec.decode("42").unwrap(), 42);
        assert!(codec.decode("not-json").is_err());
    }
}
