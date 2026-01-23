use silex_core::dom::view::{AnyView, View};
use silex_core::reactivity::{ReadSignal, WriteSignal, create_memo, provide_context, use_context};
use std::collections::HashMap;
use std::rc::Rc;
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
pub fn use_location_path() -> ReadSignal<String> {
    use_router()
        .map(|ctx| ctx.path)
        .expect("use_location_path called outside of <Router>")
}

/// Hook: 获取查询参数字符串
pub fn use_location_search() -> ReadSignal<String> {
    use_router()
        .map(|ctx| ctx.search)
        .expect("use_location called outside of <Router>")
}

/// Hook: 获取并解析查询参数为 Map
pub fn use_query_map() -> ReadSignal<HashMap<String, String>> {
    let search_signal = use_location_search();
    create_memo(move || {
        let s = search_signal.get();
        let mut map = HashMap::new();
        let clean = s.trim_start_matches('?');
        if clean.is_empty() {
            return map;
        }

        for pair in clean.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                let k = js_sys::decode_uri_component(key)
                    .ok()
                    .and_then(|x| x.as_string())
                    .unwrap_or(key.to_string());
                let v = js_sys::decode_uri_component(value)
                    .ok()
                    .and_then(|x| x.as_string())
                    .unwrap_or(value.to_string());
                map.insert(k, v);
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
    use silex_core::reactivity::{create_effect, create_rw_signal};

    let key = key.into();
    let query_map = use_query_map();
    let navigator = use_navigate();

    // 初始化：从 URL 获取初始值，如果是空则为空字符串
    let initial_value = query_map
        .get_untracked()
        .get(&key)
        .cloned()
        .unwrap_or_default();

    let signal = create_rw_signal(initial_value);

    // 监听 URL 变化 -> 更新 Signal
    // 我们需要避免回环，所以只有当值真正改变时才 set
    create_effect({
        let key = key.clone();
        let signal = signal;
        move || {
            let map = query_map.get();
            let url_val = map.get(&key).map(|s| s.as_str()).unwrap_or("");
            if signal.get_untracked() != url_val {
                signal.set(url_val.to_string());
            }
        }
    });

    // 监听 Signal 变化 -> 更新 URL
    create_effect(move || {
        let val = signal.get();
        let current_map = query_map.get_untracked();
        let current_url_val = current_map.get(&key).map(|s| s.as_str()).unwrap_or("");

        // 只有当 Signal 的值与 URL 不一致时，才推入新历史记录
        if val != current_url_val {
            let window = web_sys::window().unwrap();
            let location = window.location();
            let pathname = location.pathname().unwrap_or_else(|_| "/".into());
            let search = location.search().unwrap_or_default(); // 包含 '?'

            // 使用 URLSearchParams API 会更稳健，但为了减少依赖，我们手动处理或使用 web_sys
            // 这里我们使用 web_sys::UrlSearchParams
            if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
                if val.is_empty() {
                    params.delete(&key);
                } else {
                    params.set(&key, &val);
                }

                let new_search = params.to_string().as_string().unwrap_or_default();
                let new_url = if new_search.is_empty() {
                    pathname
                } else {
                    format!("{}?{}", pathname, new_search)
                };

                navigator.push(&new_url);
            }
        }
    });

    signal
}
