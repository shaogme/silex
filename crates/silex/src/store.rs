/// 状态管理 Trait
///
/// 为全局状态提供统一的 Thread Local 注入和获取接口。
/// 配合 `#[derive(Store)]` 宏使用可获得最佳体验。
pub trait Store: Sized + Clone + 'static {
    /// 从 Thread Local 中获取 Store 实例
    ///
    /// # Panics
    ///
    /// 如果未找到该 Store，将会 panic。
    /// 使用 `try_get` 以避免 panic。
    fn get() -> Self {
        Self::try_get().expect("Store not found in thread-local storage")
    }

    /// 尝试从 Thread Local 中获取 Store 实例
    fn try_get() -> Option<Self>;

    /// 将当前 Store 实例提供给 Thread Local
    ///
    /// # Returns
    ///
    /// 返回自身，方便链式调用。
    fn provide(self) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Clone, PartialEq, Debug)]
    struct MyStore {
        value: i32,
    }

    thread_local! {
        static MY_STORE: RefCell<Option<MyStore>> = const { RefCell::new(None) };
    }

    impl Store for MyStore {
        fn try_get() -> Option<Self> {
            MY_STORE.with(|cell| cell.borrow().clone())
        }

        fn provide(self) -> Self {
            MY_STORE.with(|cell| {
                *cell.borrow_mut() = Some(self.clone());
            });
            self
        }
    }

    #[test]
    fn test_store_try_get_none() {
        let result = MyStore::try_get();
        assert_eq!(result, None);
    }

    #[test]
    fn test_store_provide_and_try_get() {
        let store = MyStore { value: 42 };
        store.clone().provide();

        let result = MyStore::try_get();
        assert_eq!(result, Some(store));
    }

    #[test]
    fn test_store_get() {
        let store = MyStore { value: 42 };
        store.clone().provide();

        let result = MyStore::get();
        assert_eq!(result, store);
    }
}
