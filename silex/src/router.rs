pub mod context;
pub mod link;
pub mod matcher;

pub use context::*;
pub use link::*;
pub use matcher::*;

use crate::dom::tag::div;
use crate::dom::view::{AnyView, IntoAnyView, View};
use crate::reactivity::{create_effect, create_signal, on_cleanup};
use crate::router::context::provide_router_context;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::Event;

use crate::dom::error_boundary::{ErrorBoundary, ErrorBoundaryProps};
use crate::error::SilexError;
/// 路由定义
use std::rc::Rc;

/// 路由定义
struct RouteDef {
    path_pattern: String,
    view_factory: Rc<dyn Fn() -> AnyView>,
    error_factory: Option<Rc<dyn Fn(SilexError) -> AnyView>>,
}

/// 路由器组件
pub struct Router {
    routes: Vec<RouteDef>,
    fallback: Option<Rc<dyn Fn() -> AnyView>>,
}

impl Router {
    /// 创建一个新的 Router
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            fallback: None,
        }
    }

    /// 添加一个路由规则
    ///
    /// # 参数
    /// * `path`: 路径模式，如 "/", "/user/:id"
    /// * `view_fn`: 返回视图的闭包
    pub fn route<V, F>(mut self, path: &str, view_fn: F) -> Self
    where
        V: View + 'static,
        F: Fn() -> V + 'static,
    {
        self.routes.push(RouteDef {
            path_pattern: path.to_string(),
            // 将具体视图类型擦除为 AnyView
            view_factory: Rc::new(move || view_fn().into_any()),
            error_factory: None,
        });
        self
    }

    /// 添加一个带有自定义错误边界的路由
    pub fn route_with_boundary<V, F, E, FE>(mut self, path: &str, view_fn: F, error_fn: FE) -> Self
    where
        V: View + 'static,
        F: Fn() -> V + 'static,
        E: View + 'static,
        FE: Fn(SilexError) -> E + 'static,
    {
        self.routes.push(RouteDef {
            path_pattern: path.to_string(),
            view_factory: Rc::new(move || view_fn().into_any()),
            error_factory: Some(Rc::new(move |err| error_fn(err).into_any())),
        });
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
}

impl View for Router {
    fn mount(self, parent: &web_sys::Node) {
        // 1. 获取 window 对象
        let window = web_sys::window().expect("no global `window` exists");
        let location = window.location();

        // 2. 初始化 Router 状态
        let initial_path = location.pathname().unwrap_or_else(|_| "/".into());
        let initial_search = location.search().unwrap_or_else(|_| "".into());

        let (path, set_path) = create_signal(initial_path);
        let (search, set_search) = create_signal(initial_search);
        let (params, set_params) = create_signal(HashMap::new());

        // 3. 提供 Context
        provide_router_context(path, search, params, set_path, set_search);

        // 4. 监听 popstate 事件 (浏览器的后退/前进按钮)
        let set_path_clone = set_path;
        let set_search_clone = set_search;

        let on_popstate = Closure::wrap(Box::new(move |_e: Event| {
            let win = web_sys::window().unwrap();
            let loc = win.location();
            if let Ok(p) = loc.pathname() {
                set_path_clone.set(p);
            }
            if let Ok(s) = loc.search() {
                set_search_clone.set(s);
            }
        }) as Box<dyn FnMut(Event)>);

        window
            .add_event_listener_with_callback("popstate", on_popstate.as_ref().unchecked_ref())
            .expect("should attach popstate listener");

        // 清理监听器
        on_cleanup(move || {
            let window = web_sys::window().unwrap();
            let _ = window.remove_event_listener_with_callback(
                "popstate",
                on_popstate.as_ref().unchecked_ref(),
            );
        });
        // 注意：Closure 需要在 cleanup 中被 drop，或者我们利用 on_cleanup 的闭包捕获它。
        // 上面的代码中 on_popstate 被 move 进了 on_cleanup，
        // 当 cleanups 执行完毕，闭包被销毁，on_popstate 也会随之 drop，这正是我们想要的。

        // 5. 创建视图容器
        // 使用 display: contents 使得 Router 本身不产生 DOM 层级影响（如果支持）
        // 或者只是一个 div
        let container = div();
        let container_node = container.dom_element.clone();
        container.mount(parent);

        // 6. 响应式路由匹配与渲染
        let routes = self.routes;
        let fallback = self.fallback;

        create_effect(move || {
            let current_path = path.get().unwrap_or_default();

            // 简单的 O(N) 匹配
            // 未来可以优化为 Radix Tree
            let mut matched_view_factory = None;
            let mut matched_params = HashMap::new();
            let mut found = false;

            for route in &routes {
                if let Some(res) = match_path(&route.path_pattern, &current_path) {
                    matched_params = res.params;
                    matched_view_factory = Some((&route.view_factory, &route.error_factory));
                    found = true;
                    break;
                }
            }

            // 更新参数信号
            // 只有当参数真正改变时，依赖参数的组件才会更新（create_signal 内部有 PartialEq 检查）
            if found {
                set_params.set(matched_params);
            } else {
                set_params.set(HashMap::new());
            }

            // 渲染视图
            // 清空容器
            container_node.set_inner_html("");

            if let Some((factory, error_factory_opt)) = matched_view_factory {
                let factory = factory.clone();
                let error_factory = error_factory_opt.clone();

                let boundary = ErrorBoundary(ErrorBoundaryProps {
                    fallback: move |err| {
                        if let Some(ef) = &error_factory {
                            (ef)(err)
                        } else {
                            div().text(format!("Router Error: {}", err)).into_any()
                        }
                    },
                    children: move || (factory)(),
                });
                boundary.mount(&container_node);
            } else if let Some(fb_factory) = &fallback {
                let view = fb_factory();
                view.mount(&container_node);
            } else {
                // Default 404 (Blank)
            }
        });
    }
}
