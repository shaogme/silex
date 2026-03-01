use serde::{Serialize, de::DeserializeOwned};
use silex_core::error::{SilexError, SilexResult};
use silex_core::reactivity::{Effect, RwSignal, on_cleanup};
use silex_core::traits::{RxData, RxGet, RxWrite};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Storage, StorageEvent};

pub mod utils {
    use super::*;

    pub fn get_storage() -> Option<Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn set_item<T: Serialize>(key: &str, value: &T) -> SilexResult<()> {
        let storage = get_storage()
            .ok_or_else(|| SilexError::Javascript("LocalStorage not available".into()))?;
        let json =
            serde_json::to_string(value).map_err(|e| SilexError::Javascript(e.to_string()))?;
        storage
            .set_item(key, &json)
            .map_err(|_| SilexError::Javascript("Failed to set item in LocalStorage".into()))?;
        Ok(())
    }

    pub fn get_item<T: DeserializeOwned>(key: &str) -> Option<T> {
        let storage = get_storage()?;
        let json = storage.get_item(key).ok().flatten()?;
        serde_json::from_str(&json).ok()
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
    T: RxData + Serialize + DeserializeOwned + Clone + PartialEq,
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
            // 只有当值真的变化时（通过 Effect 追踪），才序列化并写入
            // 注意：这里我们不检查数据是否等于存储中的值，因为 Effect 只在 signal.set 调用时运行
            let _ = utils::set_item(&key, &val);
        }
    });

    // 3. 跨标签同步：Storage Event -> Signal
    if let Some(window) = web_sys::window() {
        let key_clone = key.clone();
        let signal_clone = signal;

        let on_storage = Closure::wrap(Box::new(move |ev: StorageEvent| {
            // 只同步匹配的 Key 且由 localStorage 触发的事件
            // ev.key() 返回 Option<String>，我们将其与 Some(key_clone) 进行比较
            if ev.key().as_ref() == Some(&key_clone) {
                if let Some(new_val_str) = ev.new_value() {
                    if let Ok(new_val) = serde_json::from_str::<T>(&new_val_str) {
                        // 避免不必要的更新：如果本地值已一致，则跳过
                        if signal_clone.get_untracked() != new_val {
                            signal_clone.set(new_val);
                        }
                    }
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
