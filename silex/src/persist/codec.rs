use crate::persist::PersistenceError;
use std::fmt::Display;
use std::marker::PhantomData;
use std::str::FromStr;

pub trait PersistCodec<T>: Clone + 'static {
    fn encode(&self, value: &T) -> Result<String, String>;
    fn decode(&self, raw: &str) -> Result<T, String>;

    fn should_remove(&self, _value: &T) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StringCodec;

impl PersistCodec<String> for StringCodec {
    fn encode(&self, value: &String) -> Result<String, String> {
        Ok(value.clone())
    }

    fn decode(&self, raw: &str) -> Result<String, String> {
        Ok(raw.to_string())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParseCodec<T>(pub PhantomData<T>);

impl<T> ParseCodec<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> PersistCodec<T> for ParseCodec<T>
where
    T: Display + FromStr + Clone + 'static,
    <T as FromStr>::Err: std::fmt::Display,
{
    fn encode(&self, value: &T) -> Result<String, String> {
        Ok(value.to_string())
    }

    fn decode(&self, raw: &str) -> Result<T, String> {
        raw.parse::<T>().map_err(|err| err.to_string())
    }
}

#[cfg(feature = "persistence")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JsonCodec<T>(pub PhantomData<T>);

#[cfg(feature = "persistence")]
impl<T> JsonCodec<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[cfg(feature = "persistence")]
impl<T> PersistCodec<T> for JsonCodec<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + 'static,
{
    fn encode(&self, value: &T) -> Result<String, String> {
        let js_value = serde_wasm_bindgen::to_value(value).map_err(|err| err.to_string())?;
        js_sys::JSON::stringify(&js_value)
            .map_err(|err| format!("JSON stringify failed: {:?}", err))?
            .as_string()
            .ok_or_else(|| "JSON stringify produced non-string result".to_string())
    }

    fn decode(&self, raw: &str) -> Result<T, String> {
        let js_value =
            js_sys::JSON::parse(raw).map_err(|err| format!("JSON parse failed: {:?}", err))?;
        serde_wasm_bindgen::from_value(js_value).map_err(|err| err.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionCodec<C, T> {
    inner: C,
    marker: PhantomData<T>,
}

impl<C, T> OptionCodec<C, T> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }
}

impl<C, T> PersistCodec<Option<T>> for OptionCodec<C, T>
where
    C: PersistCodec<T>,
    T: Clone + 'static,
{
    fn encode(&self, value: &Option<T>) -> Result<String, String> {
        match value {
            Some(value) => self.inner.encode(value),
            None => Err("OptionCodec cannot encode None directly".to_string()),
        }
    }

    fn decode(&self, raw: &str) -> Result<Option<T>, String> {
        self.inner.decode(raw).map(Some)
    }

    fn should_remove(&self, value: &Option<T>) -> bool {
        value.is_none()
    }
}

pub(crate) fn map_encode_error(message: String) -> PersistenceError {
    PersistenceError::EncodeFailed(message)
}

pub(crate) fn map_decode_error(raw: &str, message: String) -> PersistenceError {
    PersistenceError::DecodeFailed {
        raw: raw.to_string(),
        message,
    }
}
