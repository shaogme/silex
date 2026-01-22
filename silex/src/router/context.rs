use crate::dom::view::{AnyView, View};
use crate::reactivity::{ReadSignal, WriteSignal, create_memo, provide_context, use_context};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
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

/// 匹配到的路由节点 (用于运行时渲染)
#[derive(Clone, PartialEq)]
pub struct MatchedRoute {
    /// 该层级匹配到的参数
    pub params: HashMap<String, String>,
    /// 该层级的视图工厂
    pub view_factory: ViewFactory,
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
    /// 聚合后的所有路径参数 (flattened params from all levels)
    pub params: ReadSignal<HashMap<String, String>>,
    /// 匹配到的路由链 (Root -> Child -> GrandChild)
    pub matches: ReadSignal<Vec<MatchedRoute>>,
    /// 导航控制器
    pub navigator: Navigator,
}

/// 导航控制器，用于执行路由跳转
#[derive(Clone)]
pub struct Navigator {
    pub(crate) base_path: String,
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

        // 更新信号
        self.set_path.set(logical_path.to_string());
        self.set_search.set(search);
    }

    /// 导航到指定路径
    pub fn push(&self, url: &str) {
        self.handle_navigation(url, false);
    }

    /// 替换当前路径
    pub fn replace(&self, url: &str) {
        self.handle_navigation(url, true);
    }
}

/// 路由上下文所需的属性集合
#[derive(Clone)]
pub(crate) struct RouterContextProps {
    pub base_path: String,
    pub path: ReadSignal<String>,
    pub search: ReadSignal<String>,
    pub params: ReadSignal<HashMap<String, String>>,
    pub matches: ReadSignal<Vec<MatchedRoute>>,
    pub set_path: WriteSignal<String>,
    pub set_search: WriteSignal<String>,
}

/// 提供路由上下文 (由 Router 组件调用)
pub(crate) fn provide_router_context(props: RouterContextProps) {
    let navigator = Navigator {
        base_path: props.base_path.clone(),
        set_path: props.set_path,
        set_search: props.set_search,
    };
    let ctx = RouterContext {
        base_path: props.base_path,
        path: props.path,
        search: props.search,
        params: props.params,
        matches: props.matches,
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

/// Hook: 获取当前路径参数
pub fn use_params() -> ReadSignal<HashMap<String, String>> {
    use_router()
        .map(|ctx| ctx.params)
        .expect("use_params called outside of <Router>")
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

/// Hook: 获取单个强类型参数 (Scheme 3)
///
/// 适用于简单的单参数场景，无需定义结构体。
///
/// # Example
/// ```rust
/// let user_id = use_param::<i32>("id");
/// ```
pub fn use_param<T>(key: &str) -> ReadSignal<Result<T, String>>
where
    T: FromStr + Clone + PartialEq + 'static,
    T::Err: std::fmt::Display,
{
    let params = use_params();
    let key = key.to_string();
    create_memo(move || {
        let p = params.get();
        match p.get(&key) {
            Some(val_str) => val_str.parse::<T>().map_err(|e| e.to_string()),
            None => Err(format!("Parameter '{}' not found", key)),
        }
    })
}

/// Hook: 将所有参数解析为结构体 (Scheme 1)
///
/// 基于 Serde，将 HashMap 参数反序列化为强类型结构体。
///
/// # Example
/// ```rust
/// #[derive(serde::Deserialize)]
/// struct Params {
///     id: i32,
///     name: String
/// }
/// let params = use_typed_params::<Params>();
/// ```
pub fn use_typed_params<T>() -> ReadSignal<Result<T, String>>
where
    T: DeserializeOwned + Clone + PartialEq + 'static,
{
    let params = use_params();
    create_memo(move || {
        let p = params.get();
        // 利用 serde_json 作为中间层进行转换: HashMap -> Value -> Struct
        // 这种方式虽然有一定开销，但对于路由参数这种小数据量场景非常简便且健壮
        let value = match serde_json::to_value(&p) {
            Ok(v) => v,
            Err(e) => return Err(format!("Serialization error: {}", e)),
        };
        serde_json::from_value(value).map_err(|e| e.to_string())
    })
}
