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
