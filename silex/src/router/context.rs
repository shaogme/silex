use crate::reactivity::{ReadSignal, WriteSignal, provide_context, use_context};
use std::collections::HashMap;

/// 路由上下文，存储当前的路由状态
#[derive(Clone)]
pub struct RouterContext {
    /// 当前路径 (pathname)
    pub path: ReadSignal<String>,
    /// 当前查询参数 (search string)
    pub search: ReadSignal<String>,
    /// 路径参数 (parsed params from URL pattern)
    pub params: ReadSignal<HashMap<String, String>>,
    /// 导航控制器
    pub navigator: Navigator,
}

/// 导航控制器，用于执行路由跳转
#[derive(Clone, Copy)]
pub struct Navigator {
    pub(crate) set_path: WriteSignal<String>,
    pub(crate) set_search: WriteSignal<String>,
}

impl Navigator {
    /// 导航到指定路径
    pub fn push(&self, url: &str) {
        let window = web_sys::window().unwrap();

        // 使用 History API 更新浏览器 URL
        // push_state(data, title, url)
        if let Ok(history) = window.history() {
            let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(url));
        }

        // 解析 URL 分离 path 和 search
        // 简单处理：找到第一个 ?
        let (path_str, search_str) = if let Some(idx) = url.find('?') {
            (&url[..idx], &url[idx..])
        } else {
            (url, "")
        };

        // 更新信号，触发 Router 重新匹配
        self.set_path.set(path_str.to_string());
        self.set_search.set(search_str.to_string());
    }
}

/// 提供路由上下文 (由 Router 组件调用)
pub(crate) fn provide_router_context(
    path: ReadSignal<String>,
    search: ReadSignal<String>,
    params: ReadSignal<HashMap<String, String>>,
    set_path: WriteSignal<String>,
    set_search: WriteSignal<String>,
) {
    let navigator = Navigator {
        set_path,
        set_search,
    };
    let ctx = RouterContext {
        path,
        search,
        params,
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
pub fn use_navigate() -> impl Fn(&str) {
    let ctx = use_router();
    move |url: &str| {
        if let Some(c) = &ctx {
            c.navigator.push(url);
        } else {
            crate::error!("use_navigate called outside of <Router>");
        }
    }
}

/// Hook: 获取当前路径参数
pub fn use_params() -> ReadSignal<HashMap<String, String>> {
    use_router()
        .map(|ctx| ctx.params)
        .expect("use_params called outside of <Router>")
}

/// Hook: 获取当前路径
pub fn use_location_path() -> ReadSignal<String> {
    use_router()
        .map(|ctx| ctx.path)
        .expect("use_location called outside of <Router>")
}

/// Hook: 获取查询参数字符串
pub fn use_location_search() -> ReadSignal<String> {
    use_router()
        .map(|ctx| ctx.search)
        .expect("use_location called outside of <Router>")
}
