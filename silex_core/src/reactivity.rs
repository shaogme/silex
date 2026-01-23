pub mod runtime;

pub use runtime::NodeId;

use std::any::TypeId;
use std::cell::Cell;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::reactivity::runtime::{RUNTIME, run_effect};
use crate::{SilexError, SilexResult};

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

/// `Resource` 用于处理异步数据加载。
/// 它包含数据信号 (`data`)、错误信号 (`error`)、加载状态信号 (`loading`) 和一个重新获取触发器。
pub struct Resource<T: 'static, E: 'static = SilexError> {
    /// 存储异步获取的数据，初始为 `None`。
    pub data: ReadSignal<Option<T>>,
    /// 存储异步获取的错误，如果成功则为 `None`。
    pub error: ReadSignal<Option<E>>,
    /// 指示数据是否正在加载中。
    pub loading: ReadSignal<bool>,
    /// 用于手动触发重新加载的信号。
    trigger: WriteSignal<usize>,
}
impl<T, E> Clone for Resource<T, E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, E> Copy for Resource<T, E> {}

/// `ResourceFetcher` trait 抽象了数据获取逻辑。
/// 这允许 create_resource 接受普通的闭包，或者自定义的实现了此 trait 的结构体（用于更复杂的类型推导场景）。
pub trait ResourceFetcher<S> {
    type Data;
    type Error;
    type Future: Future<Output = Result<Self::Data, Self::Error>>;

    fn fetch(&self, source: S) -> Self::Future;
}

impl<S, T, E, Fun, Fut> ResourceFetcher<S> for Fun
where
    Fun: Fn(S) -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    type Data = T;
    type Error = E;
    type Future = Fut;

    fn fetch(&self, source: S) -> Self::Future {
        self(source)
    }
}

/// 创建一个资源 (`Resource`)，用于管理异步数据获取。
///
/// # 参数
/// * `source` - 一个闭包，返回用于获取数据的参数（如 ID 或 URL）。它是响应式的，当返回值变化时会自动重新获取数据。
/// * `fetcher` - 数据获取器，可以是闭包 `|s| async { ... }` 或实现了 `ResourceFetcher` 的类型。
pub fn create_resource<S, Fetcher>(
    source: impl Fn() -> S + 'static,
    fetcher: Fetcher,
) -> SilexResult<Resource<Fetcher::Data, Fetcher::Error>>
where
    S: PartialEq + Clone + 'static,
    Fetcher: ResourceFetcher<S> + 'static,
    Fetcher::Data: Clone + 'static,
    Fetcher::Error: Clone + 'static + std::fmt::Debug,
{
    let (data, set_data) = create_signal(None);
    let (error, set_error) = create_signal(None);
    let (loading, set_loading) = create_signal(false);
    let (trigger, set_trigger) = create_signal(0);

    // 追踪资源所有者（通常是组件调用点）的生命周期。
    // 如果组件被卸载，我们不应该再更新状态。
    let alive = Rc::new(Cell::new(true));
    let alive_clone = alive.clone();
    on_cleanup(move || alive_clone.set(false));

    // 用于解决竞态条件：追踪最新的请求 ID
    let request_id = Rc::new(Cell::new(0usize));

    create_effect(move || {
        let source_val = source();
        // 追踪 trigger 以允许手动重新获取
        let _ = trigger.get();

        let suspense_ctx = use_suspense_context();
        if let Some(ctx) = &suspense_ctx {
            ctx.increment();
        }
        let _ = set_loading.set(true);

        // 每次发起请求前递增 ID
        let current_id = request_id.get().wrapping_add(1);
        request_id.set(current_id);

        // 启动异步任务
        let fut = fetcher.fetch(source_val);
        let suspense_ctx = suspense_ctx.clone();

        let alive = alive.clone();
        let request_id = request_id.clone();

        wasm_bindgen_futures::spawn_local(async move {
            let res = fut.await;

            // 仅当组件仍然存活 且 这是最新的请求时 更新状态
            if alive.get() && request_id.get() == current_id {
                match res {
                    Ok(val) => {
                        let _ = set_data.set(Some(val));
                        let _ = set_error.set(None);
                    }
                    Err(e) => {
                        // 出错时，是否清除旧数据取决于策略，这里暂时保留旧数据或清理
                        // let _ = set_data.set(None); // Uncomment to clear data on error
                        let _ = set_error.set(Some(e));
                    }
                }
                let _ = set_loading.set(false);
            }

            // 指示加载完成
            if let Some(ctx) = &suspense_ctx {
                ctx.decrement();
            }
        });
    });

    Ok(Resource {
        data,
        error,
        loading,
        trigger: set_trigger,
    })
}

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> Resource<T, E> {
    /// 获取资源数据。如果是 `None` 则表示尚未加载完成或初始状态。
    /// 如果有错误，会尝试上报到 ErrorBoundary。
    pub fn get(&self) -> Option<T> {
        if let Some(e) = self.error.get() {
            if let Some(ctx) = use_context::<crate::error::ErrorContext>() {
                let err_msg = format!("{:?}", e);
                (ctx.0)(crate::error::SilexError::Javascript(err_msg));
            }
        }
        self.data.get()
    }

    /// 检查资源是否正在加载。
    pub fn loading(&self) -> bool {
        self.loading.get()
    }

    /// 手动触发重新获取数据。
    pub fn refetch(&self) {
        let _ = self.trigger.update(|n| *n = n.wrapping_add(1));
    }
}

// --- Context 上下文 API ---

/// 提供一个上下文值给当前组件树及其子孙组件。
/// 上下文基于类型 (`T`) 进行键控。
pub fn provide_context<T: 'static>(value: T) -> SilexResult<()> {
    RUNTIME.with(|rt| {
        if let Some(owner) = *rt.current_owner.borrow() {
            let mut nodes = rt.nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(owner) {
                if node.context.is_none() {
                    node.context = Some(HashMap::new());
                }
                // unwrap exists now because we just checked/created it
                if let Some(ctx) = &mut node.context {
                    ctx.insert(TypeId::of::<T>(), Box::new(value));
                }
                Ok(())
            } else {
                Err(SilexError::Reactivity(
                    "provide_context owner not found".into(),
                ))
            }
        } else {
            Err(SilexError::Reactivity(
                "provide_context 被调用时没有 owner 作用域".into(),
            ))
        }
    })
}

/// 获取上下文值。
/// 会向上遍历组件树，直到找到对应类型的上下文。
pub fn use_context<T: Clone + 'static>() -> Option<T> {
    RUNTIME.with(|rt| {
        let nodes = rt.nodes.borrow();
        let mut current_opt = *rt.current_owner.borrow();

        // 向上遍历树
        while let Some(current) = current_opt {
            if let Some(node) = nodes.get(current) {
                if let Some(ctx) = &node.context {
                    if let Some(val) = ctx.get(&TypeId::of::<T>()) {
                        return val.downcast_ref::<T>().cloned();
                    }
                }
                // 移动到父节点
                current_opt = node.parent;
            } else {
                current_opt = None;
            }
        }
        None
    })
}

/// 获取上下文值，如果未找到则 Panic。
/// 适用于那些必须存在的上下文（如 Router, Theme）。
pub fn expect_context<T: Clone + 'static>() -> T {
    match use_context::<T>() {
        Some(v) => v,
        None => {
            let type_name = std::any::type_name::<T>();
            let msg = format!(
                "Expected context `{}` but none found. Did you forget to wrap your component in a Provider?",
                type_name
            );
            crate::log::console_error(&msg);
            panic!("{}", msg);
        }
    }
}

// --- Effect 副作用 API ---

/// 创建一个副作用 (Effect)。
/// 副作用是一个并在依赖发生变化时自动重新运行的闭包。
/// `f` 闭包会被立即执行一次以进行依赖收集。
pub fn create_effect<F>(f: F)
where
    F: Fn() + 'static,
{
    let id = RUNTIME.with(|rt| rt.register_effect(f));
    run_effect(id);
}

// --- Scope 作用域 API ---

/// 创建一个新的响应式作用域 (Score)。
/// 作用域用于管理资源的生命周期（如 Effect、Signal 等）。
/// 当作用域被销毁时，其下的所有资源也会被清理。
pub fn create_scope<F>(f: F) -> NodeId
where
    F: FnOnce(),
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();

        let prev_owner = *rt.current_owner.borrow();
        *rt.current_owner.borrow_mut() = Some(id);
        let _ = f();
        *rt.current_owner.borrow_mut() = prev_owner;

        id
    })
}

/// 销毁指定的作用域或节点。
/// 这会清理该节点下的所有资源和子节点。
pub fn dispose(id: NodeId) {
    RUNTIME.with(|rt| {
        rt.dispose_node(id, true);
    });
}

/// 注册一个在当前作用域被清理时执行的回调函数。
/// 这对于释放非内存资源（如定时器、DOM 事件监听器等）非常有用。
pub fn on_cleanup(f: impl FnOnce() + 'static) {
    RUNTIME.with(|rt| {
        if let Some(owner) = *rt.current_owner.borrow() {
            let mut nodes = rt.nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(owner) {
                node.cleanups.push(Box::new(f));
            }
        }
    });
}

// --- Suspense 悬念/异步等待 API ---

/// `SuspenseContext` 用于在异步操作进行时管理挂起状态。
/// 它维护一个计数器，表示当前有多少个异步任务正在进行。
#[derive(Clone, Copy)]
pub struct SuspenseContext {
    pub count: ReadSignal<usize>,
    pub set_count: WriteSignal<usize>,
}

impl SuspenseContext {
    pub fn new() -> Self {
        let (count, set_count) = create_signal(0);
        Self { count, set_count }
    }

    /// 增加挂起的任务计数。
    pub fn increment(&self) {
        // 优化：Signal 更新是同步的。
        let _ = self.set_count.update(|c| *c += 1);
    }

    /// 减少挂起的任务计数。
    pub fn decrement(&self) {
        let _ = self.set_count.update(|c| {
            if *c > 0 {
                *c -= 1
            }
        });
    }
}

/// 获取当前的 `SuspenseContext`。
/// 通常由 `Suspense` 组件提供。
pub fn use_suspense_context() -> Option<SuspenseContext> {
    use_context::<SuspenseContext>()
}
