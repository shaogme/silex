use crate::prelude::*;

/// 状态管理 Trait
///
/// 为全局或局部状态提供统一的 Context 注入和获取接口。
/// 配合 `#[derive(Store)]` 宏使用可获得最佳体验。
pub trait Store: Sized + Clone + 'static {
    /// 从当前 Context 中获取 Store 实例
    ///
    /// # Panics
    ///
    /// 如果 Context 中未找到该 Store，将会 panic。
    /// 使用 `try_get` 以避免 panic。
    fn get() -> Self {
        expect_context::<Self>()
    }

    /// 尝试从当前 Context 中获取 Store 实例
    fn try_get() -> Option<Self> {
        use_context::<Self>()
    }

    /// 将当前 Store 实例提供给组件树
    ///
    /// # Returns
    ///
    /// 返回自身，方便链式调用。
    fn provide(self) -> Self {
        provide_context(self.clone());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, Debug)]
    struct MyStore {
        value: i32,
    }

    impl Store for MyStore {}

    #[test]
    fn test_store_try_get_none() {
        create_scope(|| {
            let result = MyStore::try_get();
            assert_eq!(result, None);
        });
    }

    #[test]
    fn test_store_provide_and_try_get() {
        create_scope(|| {
            let store = MyStore { value: 42 };
            store.clone().provide();

            let result = MyStore::try_get();
            assert_eq!(result, Some(store));
        });
    }

    #[test]
    fn test_store_get() {
        create_scope(|| {
            let store = MyStore { value: 42 };
            store.clone().provide();

            let result = MyStore::get();
            assert_eq!(result, store);
        });
    }
}
