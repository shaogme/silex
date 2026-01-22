pub mod context;
pub mod link;
pub mod matcher;
pub mod outlet;
pub mod route;

pub use context::*;
pub use link::*;
pub use matcher::*;
pub use outlet::*;
pub use route::*;

use crate::dom::tag::div;
use crate::dom::view::{AnyView, IntoAnyView, View};
use crate::reactivity::{create_effect, create_signal, on_cleanup};
use crate::router::context::{RouterContextProps, provide_router_context};
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::Event;

/// 路由能力特征
///
/// 实现此特征的枚举可以作为类型安全路由使用。
/// 通常通过 `#[derive(Route)]` 宏自动实现。
pub trait Routable: Sized + Clone + PartialEq + 'static {
    /// 尝试从路径字符串匹配并解析出实例
    fn match_path(path: &str) -> Option<Self>;

    /// 将实例转换为 URL 路径字符串
    fn to_path(&self) -> String;
}

/// 路由器组件
pub struct Router {
    routes: Vec<Route>,
    fallback: Option<Rc<dyn Fn() -> AnyView>>,
    base_path: String,
}

impl Router {
    /// 创建一个新的 Router
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            fallback: None,
            base_path: "/".to_string(),
        }
    }

    /// 设置基础路径 (e.g. "/app")
    pub fn base(mut self, path: &str) -> Self {
        let mut p = path.to_string();
        if !p.starts_with('/') {
            p = format!("/{}", p);
        }
        if p.len() > 1 && p.ends_with('/') {
            p.pop();
        }
        self.base_path = p;
        self
    }

    /// 添加路由规则 (支持嵌套)
    pub fn add(mut self, route: Route) -> Self {
        self.routes.push(route);
        self
    }

    /// 为了兼容旧 API，提供 route 方法 (将被视为扁平路由，或者手动构建 Route)
    /// 建议使用 `add(Route::new(path, view))`
    pub fn route<V, F>(mut self, path: &str, view_fn: F) -> Self
    where
        V: View + 'static,
        F: Fn() -> V + 'static,
    {
        self.routes.push(Route::new(path, view_fn));
        self
    }

    /// 设置 404 回退视图
    pub fn fallback<V, F>(mut self, view_fn: F) -> Self
    where
        V: View + 'static,
        F: Fn() -> V + 'static,
    {
        self.fallback = Some(Rc::new(move || view_fn().into_any()));
        self
    }

    /// 使用实现了 Routable 的 Enum 进行强类型路由匹配
    ///
    /// 这将添加一个这一层的通配符路由 "/*"，并将路径匹配委托给 Enum 的 `match_path` 实现。
    /// 建议在使用此模式时，不要混合使用普通的 `add`。
    pub fn match_enum<R, F, V>(mut self, render: F) -> Self
    where
        R: Routable,
        F: Fn(R) -> V + 'static,
        V: View + 'static,
    {
        let render = Rc::new(render);
        self.routes.push(Route::new("/*", move || {
            let path = crate::router::use_location_path().get();
            if let Some(matched) = R::match_path(&path) {
                render(matched).into_any()
            } else {
                // 如果 Enum 没有匹配（且没定义 Fallback 变体），可以在这里处理，
                // 或者是渲染空，让 Router 的 fallback 机制处理？
                // 由于我们已经捕获了 "/*"，Router 认为我们匹配成功了。
                // 所以如果要显示 404，最好在 Enum 里定义 #[route("/*")] Not Found。
                AnyView::new(())
            }
        }));
        self
    }
}

// 递归匹配逻辑
fn match_routes(routes: &[Route], path: &str) -> Option<Vec<MatchedRoute>> {
    for route in routes {
        let is_leaf = route.children.is_empty();
        // 如果是 leaf, 必须完全匹配 (!is_leaf => partial=false, meaning strict)
        // Wait, logic: partial=true means allow suffix. partial=false means exact match.
        // If it is a leaf, it MUST be exact match (partial=false).
        // If it is NOT a leaf, it is a parent, it matches prefix (partial=true).
        let partial_match = !is_leaf;

        if let Some(res) = match_path(&route.path, path, partial_match) {
            let matched = MatchedRoute {
                params: res.params,
                view_factory: ViewFactory(route.view.clone()),
            };

            if is_leaf {
                // 叶子节点，匹配成功
                return Some(vec![matched]);
            } else {
                // 有子节点，检查剩余路径
                // 剩余路径可能是空字符串，这发生在父路由完整匹配了路径。
                // 此时应该尝试匹配子路由中的空路径 (Index Route) 或者如果找不到则视作匹配到此为止(如果业务允许)
                // 但在嵌套路由中，通常如果 URL 是 /users，Parent 是 /users，Child 是 /:id
                // 那么剩余 ""。Child :id 不匹配 ""。
                // 如果 Child 有 Route::new("", IndexView)，它匹配 ""。

                // 如果剩余路径非空，必须匹配子路由，否则此分支作废。

                // 处理子路由匹配
                if let Some(mut child_matches) = match_routes(&route.children, &res.remaining_path)
                {
                    let mut full_matches = vec![matched];
                    full_matches.append(&mut child_matches);
                    return Some(full_matches);
                } else {
                    // 没匹配到子路由。
                    // 如果剩余路径为空 (e.g. 访问了 /parent 但没有 index 路由)，我们依然算作父路由匹配成功？
                    // 是的，父路由会渲染，Outlet 为空。
                    if res.remaining_path.is_empty() || res.remaining_path == "/" {
                        return Some(vec![matched]);
                    }
                    // 否则不匹配
                    continue;
                }
            }
        }
    }
    None
}

impl View for Router {
    fn mount(self, parent: &web_sys::Node) {
        // 1. 获取 window 对象
        let window = web_sys::window().expect("no global `window` exists");
        let location = window.location();
        let raw_path = location.pathname().unwrap_or_else(|_| "/".into());
        let initial_search = location.search().unwrap_or_else(|_| "".into());
        let base_path = self.base_path.clone();

        // 1.5 初始路径处理：剥离 base_path
        let initial_path =
            if !base_path.is_empty() && base_path != "/" && raw_path.starts_with(&base_path) {
                let p = &raw_path[base_path.len()..];
                if p.is_empty() {
                    "/".to_string()
                } else {
                    p.to_string()
                }
            } else {
                raw_path
            };

        // 2. 初始化信号
        let (path, set_path) = create_signal(initial_path);
        let (search, set_search) = create_signal(initial_search);
        let (params, set_params) = create_signal(HashMap::new());
        let (matches, set_matches) = create_signal(Vec::new());

        // 3. 提供 Context
        provide_router_context(RouterContextProps {
            base_path: base_path.clone(),
            path,
            search,
            params,
            matches,
            set_path,
            set_search,
        });

        // 4. 监听 popstate
        let set_path_clone = set_path;
        let set_search_clone = set_search;
        let base_path_clone = base_path.clone();

        let on_popstate = Closure::wrap(Box::new(move |_e: Event| {
            let win = web_sys::window().unwrap();
            let loc = win.location();

            // 处理路径变化
            if let Ok(raw_p) = loc.pathname() {
                let p = if !base_path_clone.is_empty()
                    && base_path_clone != "/"
                    && raw_p.starts_with(&base_path_clone)
                {
                    let s = &raw_p[base_path_clone.len()..];
                    if s.is_empty() {
                        "/".to_string()
                    } else {
                        s.to_string()
                    }
                } else {
                    raw_p
                };
                set_path_clone.set(p);
            }

            if let Ok(s) = loc.search() {
                set_search_clone.set(s);
            }
        }) as Box<dyn FnMut(Event)>);

        window
            .add_event_listener_with_callback("popstate", on_popstate.as_ref().unchecked_ref())
            .unwrap();

        // 5. 挂载容器
        let container = div();
        let container_node = container.dom_element.clone();
        container.mount(parent);

        // 6. 清理
        on_cleanup(move || {
            let w = web_sys::window().unwrap();
            let _ = w.remove_event_listener_with_callback(
                "popstate",
                on_popstate.as_ref().unchecked_ref(),
            );
        });

        // 7. 路由匹配 Effect
        let routes = self.routes;
        // let fallback = self.fallback; // Moved to layout rendering

        create_effect(move || {
            let current_path = path.get();
            // 执行递归匹配
            let result = match_routes(&routes, &current_path);

            if let Some(matched_chain) = result {
                // 聚合参数
                let mut all_params = HashMap::new();
                for m in &matched_chain {
                    all_params.extend(m.params.clone());
                }
                set_params.set(all_params);
                set_matches.set(matched_chain);
            } else {
                set_matches.set(Vec::new());
                set_params.set(HashMap::new());
            }
        });

        // 8. 渲染 Root Outlet (Depth 0)
        let root_outlet = Outlet(); // Now returns ViewFactory (which is Clone and View)
        let fallback_opt = self.fallback;

        // 动态视图逻辑 (本身是一个闭包，实现了 View)
        let root_view_logic = move || {
            let ms = matches.get();
            if ms.is_empty() {
                if let Some(fb) = &fallback_opt {
                    fb().into_any()
                } else {
                    AnyView::new(())
                }
            } else {
                // 匹配成功，渲染 Root Outlet
                // root_outlet 是 ViewFactory，实现了 View。我们将它转为 AnyView。
                root_outlet.clone().into_any()
            }
        };

        // 挂载
        root_view_logic.mount(&container_node);
    }
}
