use silex_core::error::{SilexError, SilexResult};
use silex_core::reactivity::{Effect, RwSignal, on_cleanup};
use silex_core::traits::{RxData, RxGet, RxWrite};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Storage, StorageEvent};

/// 存储编码器接口，用于将类型与字符串互相转换。
pub trait StorageCodec: Sized {
    fn encode(&self) -> String;
    fn decode(val: &str) -> Option<Self>;
}

impl StorageCodec for String {
    fn encode(&self) -> String {
        self.clone()
    }
    fn decode(val: &str) -> Option<Self> {
        Some(val.to_string())
    }
}

macro_rules! impl_storage_codec_parse {
    ($($t:ty),*) => {
        $(
            impl StorageCodec for $t {
                fn encode(&self) -> String { self.to_string() }
                fn decode(val: &str) -> Option<Self> { val.parse().ok() }
            }
        )*
    };
}

impl_storage_codec_parse!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool
);

/// JSON 包装器，可在开启 `persistence` 特性时提供 Serde 支持。
#[cfg(feature = "persistence")]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Json<T>(pub T);

#[cfg(feature = "persistence")]
impl<T: serde::Serialize + serde::de::DeserializeOwned> StorageCodec for Json<T> {
    fn encode(&self) -> String {
        serde_wasm_bindgen::to_value(&self.0)
            .ok()
            .and_then(|v| js_sys::JSON::stringify(&v).ok())
            .and_then(|s| s.as_string())
            .unwrap_or_default()
    }
    fn decode(val: &str) -> Option<Self> {
        js_sys::JSON::parse(val)
            .ok()
            .and_then(|v| serde_wasm_bindgen::from_value(v).ok())
            .map(Json)
    }
}

pub mod utils {
    use super::*;

    pub fn get_storage() -> Option<Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn set_item<T: StorageCodec>(key: &str, value: &T) -> SilexResult<()> {
        let storage = get_storage()
            .ok_or_else(|| SilexError::Javascript("LocalStorage not available".into()))?;
        let val_str = value.encode();
        storage
            .set_item(key, &val_str)
            .map_err(|_| SilexError::Javascript("Failed to set item in LocalStorage".into()))?;
        Ok(())
    }

    pub fn get_item<T: StorageCodec>(key: &str) -> Option<T> {
        let storage = get_storage()?;
        let json = storage.get_item(key).ok().flatten()?;
        T::decode(&json)
    }

    pub fn remove_item(key: &str) {
        if let Some(storage) = get_storage() {
            let _ = storage.remove_item(key);
        }
    }
}

/// 将一个信号与 localStorage 对应的 Key 双向绑定。
///
/// 生命周期：只要该信号持有者（Scope）活跃，它就会保持同步。
/// 跨标签同步：监听浏览器的 `storage` 事件，并在发生变化时自动更新本地信号。
pub fn use_local_storage<T>(key: impl Into<String>, default: T) -> RwSignal<T>
where
    T: RxData + StorageCodec + Clone + PartialEq,
{
    let key = key.into();

    // 1. 初始化：尝试从存储读取，失败则使用默认值
    let initial_value = utils::get_item::<T>(&key).unwrap_or(default);
    let signal = RwSignal::new(initial_value);

    // 2. 持久化副作用：Signal -> Storage
    Effect::new({
        let key = key.clone();
        move |_| {
            let val = signal.get();
            // 只有当值真的变化时（通过 Effect 追踪），才写入
            let _ = utils::set_item(&key, &val);
        }
    });

    // 3. 跨标签同步：Storage Event -> Signal
    if let Some(window) = web_sys::window() {
        let key_clone = key.clone();
        let signal_clone = signal;

        let on_storage = Closure::wrap(Box::new(move |ev: StorageEvent| {
            // 只同步匹配的 Key 且由 localStorage 触发的事件
            if ev.key().as_ref() == Some(&key_clone)
                && let Some(new_val_str) = ev.new_value()
                && let Some(new_val) = T::decode(&new_val_str)
            {
                // 避免不必要的更新：如果本地值已一致，则跳过
                if signal_clone.get_untracked() != new_val {
                    signal_clone.set(new_val);
                }
            }
        }) as Box<dyn FnMut(StorageEvent)>);

        let _ =
            window.add_event_listener_with_callback("storage", on_storage.as_ref().unchecked_ref());

        on_cleanup(move || {
            if let Some(w) = web_sys::window() {
                let _ = w.remove_event_listener_with_callback(
                    "storage",
                    on_storage.as_ref().unchecked_ref(),
                );
            }
            // 显式丢弃闭包
            drop(on_storage);
        });
    }

    signal
}
