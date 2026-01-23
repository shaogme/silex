pub mod runtime;
pub mod signal;

pub use runtime::NodeId;
pub use signal::*;

use std::any::TypeId;
use std::cell::Cell;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;

use crate::reactivity::runtime::{RUNTIME, run_effect};
use crate::{SilexError, SilexResult};

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
