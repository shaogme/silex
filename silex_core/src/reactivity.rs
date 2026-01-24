use std::cell::{Cell, RefCell};
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;

pub use silex_reactivity::NodeId;
pub use silex_reactivity::{create_scope, dispose, on_cleanup, provide_context, use_context};

use crate::SilexError;

// --- Accessor Trait ---

pub trait Accessor<T> {
    fn value(&self) -> T;
}

impl<F, T> Accessor<T> for F
where
    F: Fn() -> T,
{
    fn value(&self) -> T {
        self()
    }
}

impl<T: Clone + 'static> Accessor<T> for ReadSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + 'static> Accessor<T> for RwSignal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + PartialEq + 'static> Accessor<T> for Memo<T> {
    fn value(&self) -> T {
        self.get()
    }
}

// --- Signal 信号 API ---
#[derive(Debug)]
pub enum Signal<T: 'static> {
    Read(ReadSignal<T>),
    Derived(NodeId, PhantomData<T>),
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Signal<T> {}

impl<T: Clone + 'static> Signal<T> {
    pub fn derive(f: impl Fn() -> T + 'static) -> Self {
        let id = silex_reactivity::register_derived(move || Box::new(f()));
        Signal::Derived(id, PhantomData)
    }

    pub fn get(&self) -> T {
        match self {
            Signal::Read(s) => s.get(),
            Signal::Derived(id, _) => {
                silex_reactivity::run_derived(*id).expect("Derived signal missing")
            }
        }
    }

    pub fn try_get(&self) -> Option<T> {
        match self {
            Signal::Read(s) => s.try_get(),
            Signal::Derived(id, _) => silex_reactivity::run_derived(*id),
        }
    }

    pub fn get_untracked(&self) -> T {
        match self {
            Signal::Read(s) => s.get_untracked(),
            Signal::Derived(id, _) => {
                untrack(|| silex_reactivity::run_derived(*id).expect("Derived signal missing"))
            }
        }
    }

    pub fn try_get_untracked(&self) -> Option<T> {
        match self {
            Signal::Read(s) => s.try_get_untracked(),
            Signal::Derived(id, _) => untrack(|| silex_reactivity::run_derived(*id)),
        }
    }

    pub fn with<O>(&self, f: impl FnOnce(&T) -> O) -> O {
        let val = self.get();
        f(&val)
    }
}

impl<T: Clone + 'static> Accessor<T> for Signal<T> {
    fn value(&self) -> T {
        self.get()
    }
}

impl<T: Clone + 'static> From<T> for Signal<T> {
    fn from(value: T) -> Self {
        let (read, _) = signal(value);
        Signal::Read(read)
    }
}

impl From<&str> for Signal<String> {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl<T: 'static> From<ReadSignal<T>> for Signal<T> {
    fn from(s: ReadSignal<T>) -> Self {
        Signal::Read(s)
    }
}

impl<T: 'static> From<RwSignal<T>> for Signal<T> {
    fn from(s: RwSignal<T>) -> Self {
        Signal::Read(s.read)
    }
}

impl<T: 'static> From<Memo<T>> for Signal<T> {
    fn from(m: Memo<T>) -> Self {
        Signal::Read(ReadSignal {
            id: m.id,
            marker: PhantomData,
        })
    }
}

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

pub fn signal<T: 'static>(value: T) -> (ReadSignal<T>, WriteSignal<T>) {
    let id = silex_reactivity::signal(value);
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
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    silex_reactivity::untrack(f)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Effect {
    pub(crate) id: NodeId,
}

impl Effect {
    pub fn new<T, F>(f: F) -> Self
    where
        T: 'static,
        F: Fn(Option<T>) -> T + 'static,
    {
        let val = Rc::new(RefCell::new(None::<T>));
        let val_clone = val.clone();

        let id = silex_reactivity::effect(move || {
            let old = val_clone.borrow_mut().take();
            let new = f(old);
            *val_clone.borrow_mut() = Some(new);
        });
        Effect { id }
    }

    pub fn watch<W, T, C>(deps: W, callback: C, immediate: bool) -> Self
    where
        W: Fn() -> T + 'static,
        T: Clone + PartialEq + 'static,
        C: Fn(&T, Option<&T>, Option<()>) + 'static,
    {
        let first_run = Rc::new(Cell::new(true));
        let prev_deps = Rc::new(RefCell::new(None::<T>));

        Effect::new(move |_| {
            let new_val = deps();
            let mut p_borrow = prev_deps.borrow_mut();
            let old_val = p_borrow.clone();

            let is_first = first_run.get();
            if is_first {
                first_run.set(false);
                *p_borrow = Some(new_val.clone());
                if immediate {
                    callback(&new_val, old_val.as_ref(), None);
                }
            } else {
                if old_val.as_ref() != Some(&new_val) {
                    callback(&new_val, old_val.as_ref(), None);
                    *p_borrow = Some(new_val);
                }
            }
        })
    }
}

pub struct Memo<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for Memo<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Memo({:?})", self.id)
    }
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Memo<T> {}

impl<T: Clone + PartialEq + 'static> Memo<T> {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Option<&T>) -> T + 'static,
    {
        let id = silex_reactivity::memo(f);
        Memo {
            id,
            marker: PhantomData,
        }
    }

    pub fn get(&self) -> T {
        self.try_get().expect("Memo: value has been dropped")
    }

    pub fn try_get(&self) -> Option<T> {
        silex_reactivity::try_get_signal(self.id)
    }

    pub fn get_untracked(&self) -> T {
        self.try_get_untracked()
            .expect("Memo: value has been dropped")
    }

    pub fn try_get_untracked(&self) -> Option<T> {
        silex_reactivity::try_get_signal_untracked(self.id)
    }
}

impl<T: 'static + Clone> ReadSignal<T> {
    pub fn get(&self) -> T {
        self.try_get().expect("ReadSignal: value has been dropped")
    }

    pub fn try_get(&self) -> Option<T> {
        silex_reactivity::try_get_signal(self.id)
    }

    pub fn get_untracked(&self) -> T {
        self.try_get_untracked()
            .expect("ReadSignal: value has been dropped")
    }

    pub fn try_get_untracked(&self) -> Option<T> {
        silex_reactivity::try_get_signal_untracked(self.id)
    }

    pub fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        Memo::new(move |_| f(self.get()))
    }
}

// --- Fluent API Extensions for ReadSignal ---

impl<T: Clone + 'static + PartialEq> ReadSignal<T> {
    pub fn eq<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() == other)
    }

    pub fn ne<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() != other)
    }
}

impl<T: Clone + 'static + PartialOrd> ReadSignal<T> {
    pub fn gt<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() > other)
    }

    pub fn lt<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() < other)
    }

    pub fn ge<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() >= other)
    }

    pub fn le<O>(&self, other: O) -> Memo<bool>
    where
        O: Into<T> + Clone + 'static,
    {
        let other = other.into();
        let this = *self;
        Memo::new(move |_| this.get() <= other)
    }
}

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

impl<T: Clone + 'static> RwSignal<T> {
    pub fn new(value: T) -> Self {
        let (read, write) = signal(value);
        RwSignal { read, write }
    }

    pub fn get(&self) -> T {
        self.read.get()
    }

    pub fn try_get(&self) -> Option<T> {
        self.read.try_get()
    }

    pub fn get_untracked(&self) -> T {
        self.read.get_untracked()
    }

    pub fn try_get_untracked(&self) -> Option<T> {
        self.read.try_get_untracked()
    }

    pub fn set(&self, value: T) -> () {
        self.write.set(value)
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) -> () {
        self.write.update(f)
    }

    pub fn read_signal(&self) -> ReadSignal<T> {
        self.read
    }

    pub fn write_signal(&self) -> WriteSignal<T> {
        self.write
    }

    pub fn map<U, F>(self, f: F) -> Memo<U>
    where
        F: Fn(T) -> U + 'static,
        U: Clone + PartialEq + 'static,
    {
        self.read.map(f)
    }

    pub fn setter(self, value: T) -> impl Fn()
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    pub fn updater<F>(self, f: F) -> impl Fn()
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

impl<T: 'static> WriteSignal<T> {
    pub fn set(&self, new_value: T) -> () {
        self.update(|v| *v = new_value)
    }

    pub fn update(&self, f: impl FnOnce(&mut T)) {
        silex_reactivity::update_signal(self.id, f)
    }

    pub fn setter(self, value: T) -> impl Fn()
    where
        T: Clone,
    {
        move || self.set(value.clone())
    }

    pub fn updater<F>(self, f: F) -> impl Fn()
    where
        F: Fn(&mut T) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

// --- Resource 资源 API ---

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
/// 这允许 resource 接受普通的闭包，或者自定义的实现了此 trait 的结构体（用于更复杂的类型推导场景）。
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

impl<T: Clone + 'static, E: Clone + 'static + std::fmt::Debug> Resource<T, E> {
    /// 创建一个资源 (`Resource`)，用于管理异步数据获取。
    ///
    /// # 参数
    /// * `source` - 一个闭包，返回用于获取数据的参数（如 ID 或 URL）。它是响应式的，当返回值变化时会自动重新获取数据。
    /// * `fetcher` - 数据获取器，可以是闭包 `|s| async { ... }` 或实现了 `ResourceFetcher` 的类型。
    pub fn new<S, Fetcher>(source: impl Fn() -> S + 'static, fetcher: Fetcher) -> Self
    where
        S: PartialEq + Clone + 'static,
        Fetcher: ResourceFetcher<S, Data = T, Error = E> + 'static,
    {
        let (data, set_data) = signal(None);
        let (error, set_error) = signal(None);
        let (loading, set_loading) = signal(false);
        let (trigger, set_trigger) = signal(0);

        // 追踪资源所有者（通常是组件调用点）的生命周期。
        // 如果组件被卸载，我们不应该再更新状态。
        let alive = Rc::new(Cell::new(true));
        let alive_clone = alive.clone();
        on_cleanup(move || alive_clone.set(false));

        // 用于解决竞态条件：追踪最新的请求 ID
        let request_id = Rc::new(Cell::new(0usize));

        Effect::new(move |_| {
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

        Resource {
            data,
            error,
            loading,
            trigger: set_trigger,
        }
    }

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
        let (count, set_count) = signal(0);
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

// --- StoredValue 存储值 API ---

/// `StoredValue` 是一种非响应式的数据存储容器。
/// 它的数据存储在响应式系统中，随 Owner (Scope/Effect) 一起自动释放，
/// 但其读写操作**不会**触发任何响应式更新。
///
/// 适用于存储定时器句柄、不需要驱动 UI 的大型数据结构等。
pub struct StoredValue<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for StoredValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredValue({:?})", self.id)
    }
}

impl<T> Clone for StoredValue<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for StoredValue<T> {}

impl<T: 'static> StoredValue<T> {
    /// 创建一个新的 `StoredValue`。
    pub fn new(value: T) -> Self {
        let id = silex_reactivity::store_value(value);
        Self {
            id,
            marker: PhantomData,
        }
    }

    /// 设置新值。
    pub fn set_value(&self, value: T) {
        self.update_value(|v| *v = value);
    }

    /// 原地修改值。
    /// 注意：这**不会**通知任何订阅者，因为 StoredValue 是非响应式的。
    pub fn update_value(&self, f: impl FnOnce(&mut T)) {
        silex_reactivity::try_update_stored_value(self.id, f);
    }

    /// 以不可变借用的方式访问值。
    /// 如果值已被释放，则 Panic。
    pub fn with_value<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.try_with_value(f)
            .expect("StoredValue: value has been dropped")
    }

    /// 尝试以不可变借用的方式访问值。
    pub fn try_with_value<U>(&self, f: impl FnOnce(&T) -> U) -> Option<U> {
        silex_reactivity::try_with_stored_value(self.id, f)
    }
}

impl<T: Clone + 'static> StoredValue<T> {
    /// 获取值的克隆。
    pub fn get_value(&self) -> T {
        self.with_value(|v| v.clone())
    }

    /// 尝试获取值的克隆。
    pub fn try_get_value(&self) -> Option<T> {
        self.try_with_value(|v| v.clone())
    }
}

impl<T: Clone + 'static> Accessor<T> for StoredValue<T> {
    fn value(&self) -> T {
        self.get_value()
    }
}
