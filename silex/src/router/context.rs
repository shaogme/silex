use silex_core::reactivity::{Memo, ReadSignal, Signal, WriteSignal, provide_context, use_context};
use silex_core::traits::{Get, GetUntracked, Set};
use silex_dom::view::{AnyView, View};
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::Node;

/// View 工厂包装器，必须实现 PartialEq 以便在 Signal/Memo 中使用
#[derive(Clone)]
pub struct ViewFactory(pub Rc<dyn Fn() -> AnyView>);

impl PartialEq for ViewFactory {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl View for ViewFactory {
    fn mount(self, parent: &Node) {
        // 创建闭包，利用 View for F 的已有逻辑
        // 我们需要构造一个 Fn() -> AnyView 的闭包
        let factory = self.0.clone();
        let closure = move || (factory)();
        closure.mount(parent);
    }
}

/// 路由上下文，存储当前的路由状态
#[derive(Clone)]
pub struct RouterContext {
    /// 基础路径 (e.g. "/app")
    pub base_path: String,
    /// 当前路径 (pathname, relative to base_path)
    pub path: ReadSignal<String>,
    /// 当前查询参数 (search string)
    pub search: ReadSignal<String>,
    /// 导航控制器
    pub navigator: Navigator,
}

/// 导航控制器，用于执行路由跳转
#[derive(Clone)]
pub struct Navigator {
    pub(crate) base_path: String,
    pub(crate) path: ReadSignal<String>,
    pub(crate) search: ReadSignal<String>,
    pub(crate) set_path: WriteSignal<String>,
    pub(crate) set_search: WriteSignal<String>,
}

impl Navigator {
    fn handle_navigation(&self, url: &str, replace: bool) {
        let window = web_sys::window().unwrap();

        // 1. 构造用于浏览器历史记录的完整 URL
        let full_url = if url.starts_with('/') {
            if self.base_path == "/" || self.base_path.is_empty() {
                url.to_string()
            } else {
                let base = self.base_path.trim_end_matches('/');
                format!("{}{}", base, url)
            }
        } else {
            url.to_string()
        };

        // 2. 使用 History API
        if let Ok(history) = window.history() {
            if replace {
                let _ = history.replace_state_with_url(
                    &wasm_bindgen::JsValue::NULL,
                    "",
                    Some(&full_url),
                );
            } else {
                let _ =
                    history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&full_url));
            }
        }

        // 3. 读取当前状态并更新信号 (需要剥离 base_path)
        let location = window.location();
        let raw_path = location.pathname().unwrap_or_else(|_| "/".to_string());

        let logical_path = if !self.base_path.is_empty()
            && self.base_path != "/"
            && raw_path.starts_with(&self.base_path)
        {
            let p = &raw_path[self.base_path.len()..];
            if p.is_empty() { "/" } else { p }
        } else {
            &raw_path
        };

        let search = location.search().unwrap_or_default();

        // 更新信号 (带去重，避免不必要的副作用)
        // 核心修复：Silex 的 WriteSignal.set 默认不检查 Equality，
        // 导致只要调用 set 就会触发 Router 重渲染，Input 失去焦点。
        // 这里我们手动检查相等性。
        if self.path.get_untracked() != logical_path {
            self.set_path.set(logical_path.to_string());
        }

        if self.search.get_untracked() != search {
            self.set_search.set(search);
        }
    }

    /// 导航到指定路径
    pub fn push<T: crate::router::ToRoute>(&self, to: T) {
        self.handle_navigation(&to.to_route(), false);
    }

    /// 替换当前路径
    pub fn replace<T: crate::router::ToRoute>(&self, to: T) {
        self.handle_navigation(&to.to_route(), true);
    }

    /// 设置或更新查询参数
    ///
    /// * `key`: 参数名
    /// * `value`: 参数值。如果为 `None`，则删除该参数。
    pub fn set_query(&self, key: &str, value: Option<&str>) {
        let current_search = self.search.get_untracked();

        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&current_search) {
            match value {
                Some(v) => params.set(key, v),
                None => params.delete(key),
            }

            let new_search = params.to_string().as_string().unwrap_or_default();

            // 如果 search 没变，无需导航
            // 注意：UrlSearchParams.to_string() 会标准化编码，所以即使逻辑没变，字符串也可能变化（例如顺序）
            // 但这里我们主要关心键值对的变更。
            // 既然是 set_query 显式调用，通常意味着意图变更。

            let pathname = self.path.get_untracked();
            // path signal 是逻辑路径 (不含 base)，Navigator.push 会自动处理 base_path
            // 但我们需要构造完整的 URL (path + search) 传给 push
            let new_url = if new_search.is_empty() {
                pathname
            } else {
                format!("{}?{}", pathname, new_search)
            };

            self.push(&new_url);
        }
    }
}

/// 路由上下文所需的属性集合
#[derive(Clone)]
pub(crate) struct RouterContextProps {
    pub base_path: String,
    pub path: ReadSignal<String>,
    pub search: ReadSignal<String>,
    pub set_path: WriteSignal<String>,
    pub set_search: WriteSignal<String>,
}

/// 提供路由上下文 (由 Router 组件调用)
pub(crate) fn provide_router_context(props: RouterContextProps) {
    let navigator = Navigator {
        base_path: props.base_path.clone(),
        path: props.path,
        search: props.search,
        set_path: props.set_path,
        set_search: props.set_search,
    };
    let ctx = RouterContext {
        base_path: props.base_path,
        path: props.path,
        search: props.search,
        navigator,
    };
    // 忽略可能的错误（如重复 provide），Router 应该是根级的
    let _ = provide_context(ctx);
}

/// 获取路由上下文
pub fn use_router() -> Option<RouterContext> {
    use_context::<RouterContext>()
}

/// Hook: 获取当前导航器
pub fn use_navigate() -> Navigator {
    use_router()
        .expect("use_navigate called outside of <Router>")
        .navigator
}

/// Hook: 获取当前路径 (逻辑路径，不含 Base Path)
pub fn use_location_path() -> Signal<String> {
    use_router()
        .map(|ctx| ctx.path.into())
        .expect("use_location_path called outside of <Router>")
}

/// Hook: 获取查询参数字符串
pub fn use_location_search() -> Signal<String> {
    use_router()
        .map(|ctx| ctx.search.into())
        .expect("use_location called outside of <Router>")
}

/// Hook: 获取并解析查询参数为 Map
///
/// 使用 `web_sys::UrlSearchParams` 进行标准化的解析，确保与浏览器的行为一致。
pub fn use_query_map() -> silex_core::reactivity::Memo<HashMap<String, String>> {
    let search_signal = use_location_search();
    Memo::new(move |_| {
        let s = search_signal.get();
        let mut map = HashMap::new();

        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&s) {
            // UrlSearchParams 是 Iterable，可以使用 js_sys::try_iter
            if let Ok(Some(iter)) = js_sys::try_iter(&params) {
                for item in iter {
                    if let Ok(val) = item {
                        // 迭代出的每一项都是 [key, value] 数组
                        let pair: js_sys::Array = val.unchecked_into();
                        let k = pair.get(0).as_string().unwrap_or_default();
                        let v = pair.get(1).as_string().unwrap_or_default();
                        map.insert(k, v);
                    }
                }
            }
        }
        map
    })
}

/// Hook: 双向绑定 Signal 和 URL 查询参数
///
/// 当 Signal 变化时，自动更新 URL 查询参数。
/// 当 URL 查询参数变化时，自动更新 Signal。
///
/// # 参数
/// * `key` - 查询参数的键名
///
/// # 返回
/// 一个 RwSignal，读写它会自动同步到 URL
pub fn use_query_signal(key: impl Into<String>) -> silex_core::reactivity::RwSignal<String> {
    use silex_core::reactivity::{Effect, RwSignal, StoredValue};
    use silex_core::traits::{GetUntracked, SetUntracked};

    let key = key.into();
    let query_map = use_query_map();
    let navigator = use_navigate();

    // 初始化：从 Map 获取 (此时 Map 已经是 decode 过的了)
    let initial_value = query_map
        .get_untracked()
        .get(&key)
        .cloned()
        .unwrap_or_default();

    let signal = RwSignal::new(initial_value.clone());

    // 用于打破循环引用的缓存：存储上一次我们因 "Signal 改变" 而推送到 URL 的值。
    // 如果 URL 变回这个值，我们知道这是我们自己触发的变更回响，无需再次更新 Signal。
    let last_synced_value = StoredValue::<String>::new(initial_value);

    // URL -> Signal
    // 监听 Query Map 的变化并同步到 Signal
    Effect::new({
        let key = key.clone();
        let signal = signal;
        move |_| {
            let map = query_map.get();
            let url_val = map.get(&key).map(|s| s.as_str()).unwrap_or("");

            // 只有当 URL 的值与 Signal 当前值不一致，且与我们刚写入的值也不一致时，才更新 Signal
            // 这样可以防止：Signal 设置 "A" -> URL 变 "A" -> Effect 读 "A" -> Signal 再设一遍 "A" (虽然 Signal 内部有 check，但减少一次 set 调用总是好的)
            let current_signal_val = signal.get_untracked();

            // 使用 try_get_untracked 避免 panic (虽然在此上下文通常不会 disposed)
            if let Some(last_val) = last_synced_value.try_get_untracked() {
                if current_signal_val != url_val && last_val != url_val {
                    signal.set(url_val.to_string());
                    // 更新 last_synced_value，表示这个值是来自 URL 的最新状态
                    // 使用 try_set_untracked 避免 panic
                    let _ = last_synced_value.try_set_untracked(url_val.to_string());
                }
            }
        }
    });

    // Signal -> URL
    // 监听 Signal 的变化并使用 Navigator 更新 URL
    Effect::new(move |_| {
        let val = signal.get();
        let current_map = query_map.get_untracked();
        let current_url_val = current_map.get(&key).map(|s| s.as_str()).unwrap_or("");

        // 只有当 Signal 的值与 URL 当前值不一致时，才发起导航
        if val != current_url_val {
            // 更新缓存，标记这次变更是由 Signal 发起的
            // 使用 try_set_untracked 避免 panic，如果已 disposed 则无需更新缓存
            let _ = last_synced_value.try_set_untracked(val.clone());

            let val_to_set = if val.is_empty() {
                None
            } else {
                Some(val.as_str())
            };
            navigator.set_query(&key, val_to_set);
        }
    });

    signal
}
