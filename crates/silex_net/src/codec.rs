use crate::NetError;
#[cfg(feature = "json")]
use std::marker::PhantomData;

pub trait ResponseCodec<T>: Clone + 'static {
    fn decode(&self, raw: &str) -> Result<T, NetError>;
}

#[cfg(feature = "persist")]
pub trait CacheCodec<T>: ResponseCodec<T> {
    fn build_cache(key: String, default: T) -> silex_persist::Persistent<T>;
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
    fn build_cache(key: String, default: String) -> silex_persist::Persistent<String> {
        silex_persist::Persistent::builder(key)
            .local()
            .string()
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
    fn build_cache(key: String, default: T) -> silex_persist::Persistent<T> {
        silex_persist::Persistent::builder(key)
            .local()
            .json::<T>()
            .default(default)
            .build()
    }
}
