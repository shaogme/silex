use crate::traits::RxBase;

/// 统一写入与通知 Trait (Unified Write and Notification).
/// 向上整合了所有更新、替换及通知机制，开发者只需实现最基础的闭包突变和通知接口。
pub trait RxWrite: RxBase {
    /// 仅应用可变闭包更变数据，不通知任何订阅者。（底层无感更新）
    /// 如果目标已被 disposed，则返回 None。
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet>;

    /// 手动向所有依赖此节点的订阅者发送数据变更通知。
    fn rx_notify(&self);

    // ==========================================
    // 便利 Blanket API (由框架提供默认实现)
    // ==========================================

    /// 响应式更新：使用闭包就地修改数据，并在完成后触发通知。
    #[track_caller]
    fn update(&self, fun: impl FnOnce(&mut Self::Value)) {
        self.try_update(fun).unwrap_or_else(unwrap_rx!(self))
    }

    /// 尝试响应式更新：被销毁时返回 None。
    #[track_caller]
    fn try_update<URet>(&self, fun: impl FnOnce(&mut Self::Value) -> URet) -> Option<URet> {
        let res = self.rx_try_update_untracked(fun)?;
        self.rx_notify();
        Some(res)
    }

    /// 响应式替换：直接用新数据覆盖原有的值，然后触发通知。
    #[track_caller]
    fn set(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update(|v| *v = value);
    }

    /// 尝试响应式替换。
    #[track_caller]
    fn try_set(&self, value: Self::Value) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        if self.is_disposed() {
            Some(value)
        } else {
            self.set(value);
            None
        }
    }

    /// 根据条件触发修改与通知。闭包返回 true 才会触发 notify。
    #[track_caller]
    fn maybe_update(&self, fun: impl FnOnce(&mut Self::Value) -> bool) {
        if let Some(should_notify) = self.rx_try_update_untracked(fun) {
            if should_notify {
                self.rx_notify();
            }
        }
    }

    /// 静默更新：使用闭包就地修改数据，但【不触发通知】。
    #[track_caller]
    fn update_untracked<URet>(&self, fun: impl FnOnce(&mut Self::Value) -> URet) -> URet {
        self.rx_try_update_untracked(fun)
            .unwrap_or_else(unwrap_rx!(self))
    }

    /// 尝试静默更新。
    #[track_caller]
    fn try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        self.rx_try_update_untracked(fun)
    }

    /// 静默替换：直接覆写新数据，【不触发通知】。
    #[track_caller]
    fn set_untracked(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update_untracked(|v| *v = value);
    }

    /// 尝试静默替换。
    #[track_caller]
    fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        if self.is_disposed() {
            Some(value)
        } else {
            self.set_untracked(value);
            None
        }
    }

    /// 独立触发变更通知。
    #[track_caller]
    fn notify(&self) {
        self.rx_notify();
    }

    /// 返回一个闭包，调用时会将信号设置为指定值。
    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static
    where
        Self: Sized + Clone + 'static,
        Self::Value: Sized + Clone,
    {
        move || self.set(value.clone())
    }

    /// 返回一个闭包，调用时会使用提供的函数更新信号。
    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        Self: Sized + Clone + 'static,
        Self::Value: Sized,
        F: Fn(&mut Self::Value) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}
