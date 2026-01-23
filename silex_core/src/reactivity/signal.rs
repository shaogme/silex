use std::marker::PhantomData;
use std::rc::Rc;

use crate::reactivity::runtime::{NodeId, RUNTIME};

// --- Accessor Trait ---

/// A trait that provides a uniform way to access a value,
/// abstracting over Signals, Memos, and closures.
pub trait Accessor<T> {
    fn value(&self) -> T;
}

// 1. Implementation for Closures
impl<F, T> Accessor<T> for F
where
    F: Fn() -> T,
{
    fn value(&self) -> T {
        self()
    }
}

// 2. Implementation for ReadSignal
impl<T: Clone + 'static> Accessor<T> for ReadSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

// 3. Implementation for RwSignal
impl<T: Clone + 'static> Accessor<T> for RwSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

// --- Signal 信号 API ---

/// `ReadSignal` 是一个用于读取响应式数据的句柄。
/// 它实现了 `Copy` 和 `Clone`，因此可以廉价地在闭包之间传递。
/// 当从 `ReadSignal` 读取值时，会自动追踪当前的副作用上下文（Effect）。
pub struct ReadSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for ReadSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReadSignal({:?})", self.id)
    }
}

impl<T> Clone for ReadSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ReadSignal<T> {}

/// `WriteSignal` 是一个用于写入/更新响应式数据的句柄。
/// 它也实现了 `Copy` 和 `Clone`。
/// 更新 `WriteSignal` 的值会触发所有依赖于对应 `ReadSignal` 的副作用（Effect）。
pub struct WriteSignal<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for WriteSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WriteSignal({:?})", self.id)
    }
}

impl<T> Clone for WriteSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for WriteSignal<T> {}

/// 创建一个新的 Signal（信号）。
/// 返回一个包含读取句柄 (`ReadSignal`) 和写入句柄 (`WriteSignal`) 的元组。
///
/// # 参数
/// * `value` - Signal 的初始值。
///
/// # 泛型
/// * `T` - 存储在 Signal 中的数据类型，必须满足 `'static` 生命周期。
pub fn create_signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    RUNTIME.with(|rt| {
        let id = rt.register_signal(value);
        (
            ReadSignal {
                id,
                marker: PhantomData,
            },
            WriteSignal {
                id,
                marker: PhantomData,
            },
        )
    })
}

/// 在不通过响应式系统追踪依赖的情况下运行一个闭包。
/// 这意味着在这个闭包内部读取 Signal 不会将当前的副作用注册为依赖。
///
/// # 参数
/// * `f` - 要执行的闭包。
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.with(|rt| {
        // 暂时移除当前的 owner (Effect/Scope)，以避免追踪
        let prev_owner = *rt.current_owner.borrow();
        *rt.current_owner.borrow_mut() = None;
        let t = f();
        // 恢复之前的 owner
        *rt.current_owner.borrow_mut() = prev_owner;
        t
    })
}

/// 创建一个 Memo（派生信号）。
/// Memo 是一个计算属性，它依赖于其他 Signal，并且只有当其依赖发生变化且计算结果改变时，才会通知下游。
///
/// # 参数
/// * `f` - 计算函数，用于生成新的值。
///
/// # 泛型
/// * `T` - 计算结果的类型，需要实现 `Clone` 和 `PartialEq` 以支持变更检测。
pub fn create_memo<T, F>(f: F) -> ReadSignal<T>
where
    T: Clone + PartialEq + 'static,
    F: Fn() -> T + 'static,
{
    RUNTIME.with(|rt| {
        // 1. 注册 Effect 节点，但先不提供计算函数
        let effect_id = rt.register_node();

        // 2. 在 EffectData 中占位，以便 track_dependency 可以工作
        rt.effects.borrow_mut().insert(
            effect_id,
            crate::reactivity::runtime::EffectData {
                computation: None,
                dependencies: Vec::new(),
                effect_version: 0,
            },
        );

        // 3. 运行一次 f() 来获取初始值并收集依赖
        let value = {
            let prev_owner = *rt.current_owner.borrow();
            *rt.current_owner.borrow_mut() = Some(effect_id);
            let v = f();
            *rt.current_owner.borrow_mut() = prev_owner;
            v
        };

        // 4. 创建存储值的 Signal
        let (read, write) = create_signal(value);

        // 5. 构造真正的计算闭包，用于后续更新
        let computation = move || {
            let new_value = f();
            if let Some(old_value) = read.try_get_untracked()
                && new_value != old_value
            {
                write.set(new_value);
            }
        };

        // 6. 更新 EffectData 的 computation
        // 注意：我们必须手动把这个闭包强转为 runtime 期望的类型 ()
        // Runtime expects: Option<Rc<dyn Fn() -> ()>>
        // 我们的 computation 是 Fn() -> () (因为 write.set 返回 ())

        if let Some(effect_data) = rt.effects.borrow_mut().get_mut(effect_id) {
            effect_data.computation = Some(Rc::new(computation));
        }

        read
    })
}

impl<T: 'static + Clone> ReadSignal<T> {
    /// 获取 Signal 的当前值，并追踪依赖。
    /// 如果在 Effect 上下文中调用，该 Effect 会被注册为依赖。
    /// 如果 Signal 已被销毁，此方法会 Panic。
    pub fn get(&self) -> T {
        self.try_get().expect("ReadSignal: value has been dropped")
    }

    /// 获取 Signal 的当前值，并追踪依赖。
    /// 返回 Option，如果 Signal 已被销毁则返回 None。
    pub fn try_get(&self) -> Option<T> {
        RUNTIME.with(|rt| {
            rt.track_dependency(self.id);
            self.try_get_untracked_internal(rt)
        })
    }

    /// 获取 Signal 的当前值，但不追踪依赖。
    /// 如果 Signal 已被销毁，此方法会 Panic。
    pub fn get_untracked(&self) -> T {
        self.try_get_untracked()
            .expect("ReadSignal: value has been dropped")
    }

    /// 获取 Signal 的当前值，但不追踪依赖。
    /// 返回 Option，如果 Signal 已被销毁则返回 None。
    pub fn try_get_untracked(&self) -> Option<T> {
        RUNTIME.with(|rt| self.try_get_untracked_internal(rt))
    }

    /// 内部使用的获取值方法，不涉及依赖追踪逻辑。
    fn try_get_untracked_internal(&self, rt: &crate::reactivity::runtime::Runtime) -> Option<T> {
        let signals = rt.signals.borrow();
        if let Some(signal) = signals.get(self.id) {
            let any_val = &signal.value;
            if let Some(val) = any_val.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                crate::error!("ReadSignal Type Mismatch");
                return None;
            }
        }
        // crate::error!("ReadSignal refers to dropped value");
        None
    }

    /// 创建一个新的派生信号 (Memo)，通过映射函数转换当前信号的值。
    ///
    /// # 示例
    /// ```ignore
    /// let count = create_signal(1).0;
    /// let double = count.map(|n| n * 2);
    /// ```
    pub fn map<U, F>(self, f: F) -> ReadSignal<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        create_memo(move || f(self.get()))
    }
}

// --- Fluent API Extensions for ReadSignal ---

impl<T: Clone + 'static + PartialEq> ReadSignal<T> {
    /// 创建一个布尔信号，当当前信号的值等于给定值时为 true。
    pub fn eq<O>(&self, other: O) -> ReadSignal<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        create_memo(move || this.get() == other)
    }

    /// 创建一个布尔信号，当当前信号的值不等于给定值时为 true。
    pub fn ne<O>(&self, other: O) -> ReadSignal<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        create_memo(move || this.get() != other)
    }
}

impl<T: Clone + 'static + PartialOrd> ReadSignal<T> {
    /// 创建一个布尔信号，当当前信号的值大于给定值时为 true。
    pub fn gt<O>(&self, other: O) -> ReadSignal<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        create_memo(move || this.get() > other)
    }

    /// 创建一个布尔信号，当当前信号的值小于给定值时为 true。
    pub fn lt<O>(&self, other: O) -> ReadSignal<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        create_memo(move || this.get() < other)
    }

    /// 创建一个布尔信号，当当前信号的值大于等于给定值时为 true。
    pub fn ge<O>(&self, other: O) -> ReadSignal<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        create_memo(move || this.get() >= other)
    }

    /// 创建一个布尔信号，当当前信号的值小于等于给定值时为 true。
    pub fn le<O>(&self, other: O) -> ReadSignal<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        create_memo(move || this.get() <= other)
    }
}

/// `RwSignal` 是一个读写信号，同时包含了读取和写入的功能。
/// 它是 `ReadSignal` 和 `WriteSignal` 的组合封装。
pub struct RwSignal<T: 'static> {
    pub read: ReadSignal<T>,
    pub write: WriteSignal<T>,
}

impl<T> Clone for RwSignal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for RwSignal<T> {}

/// 创建一个 `RwSignal` (读写信号)。
pub fn create_rw_signal<T: 'static>(value: T) -> RwSignal<T> {
    let (read, write) = create_signal(value);
    RwSignal { read, write }
}

impl<T: Clone + 'static> RwSignal<T> {
    /// 创建一个新的 `RwSignal` 实例。
    pub fn new(value: T) -> Self {
        create_rw_signal(value)
    }

    /// 获取值并追踪依赖 (同 `ReadSignal::get`)。
    pub fn get(&self) -> T {
        self.read.get()
    }

    /// 尝试获取值并追踪依赖 (同 `ReadSignal::try_get`)。
    pub fn try_get(&self) -> Option<T> {
        self.read.try_get()
    }

    /// 获取值但不追踪依赖 (同 `ReadSignal::get_untracked`)。
    pub fn get_untracked(&self) -> T {
        self.read.get_untracked()
    }

    /// 尝试获取值但不追踪依赖 (同 `ReadSignal::try_get_untracked`)。
    pub fn try_get_untracked(&self) -> Option<T> {
        self.read.try_get_untracked()
    }

    /// 设置新值 (同 `WriteSignal::set`)。
    pub fn set(&self, value: T) -> () {
        self.write.set(value)
    }

    /// 更新值 (同 `WriteSignal::update`)。
    pub fn update(&self, f: impl FnOnce(&mut T)) -> () {
        self.write.update(f)
    }

    /// 获取底层的 `ReadSignal`。
    pub fn read_signal(&self) -> ReadSignal<T> {
        self.read
    }

    /// 获取底层的 `WriteSignal`。
    pub fn write_signal(&self) -> WriteSignal<T> {
        self.write
    }

    /// 创建一个新的派生信号 (Memo)，通过映射函数转换当前信号的值。
    pub fn map<U, F>(self, f: F) -> ReadSignal<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        self.read.map(f)
    }

    /// 生成一个设置值的闭包。
    ///
    /// # 示例
    /// ```ignore
    /// button("Reset").on_click(count.setter(0));
    /// ```
    pub fn setter(self, value: T) -> impl Fn()
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    /// 生成一个更新值的闭包。
    ///
    /// # 示例
    /// ```ignore
    /// button("Inc").on_click(count.updater(|n| *n += 1));
    /// ```
    pub fn updater<F>(self, f: F) -> impl Fn()
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

impl<T: 'static> WriteSignal<T> {
    /// 设置 Signal 的新值。
    /// 这将通知所有依赖此 Signal 的副作用进行更新。
    pub fn set(&self, new_value: T) -> () {
        self.update(|v| *v = new_value)
    }

    /// 使用闭包更新 Signal 的值。
    /// 允许就地修改内部值。
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        RUNTIME.with(|rt| {
            // 1. 更新值
            {
                let mut signals = rt.signals.borrow_mut();
                if let Some(signal) = signals.get_mut(self.id) {
                    let any_val = &mut signal.value;
                    if let Some(val) = any_val.downcast_mut::<T>() {
                        f(val);
                    } else {
                        crate::error!("WriteSignal Type Mismatch");
                        return;
                    }
                } else {
                    crate::error!("WriteSignal refers to dropped value (no node)");
                    return;
                }
            }

            // 2. 将依赖加入队列
            rt.queue_dependents(self.id);

            // 3. 尝试运行队列
            rt.run_queue();
        })
    }

    /// 生成一个设置值的闭包。
    pub fn setter(self, value: T) -> impl Fn()
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    /// 生成一个更新值的闭包。
    pub fn updater<F>(self, f: F) -> impl Fn()
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}
