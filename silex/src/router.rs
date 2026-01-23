pub mod context;
pub mod link;

pub use context::*;
pub use link::*;

use crate::router::context::{RouterContextProps, provide_router_context};
use silex_core::reactivity::{create_effect, create_signal, on_cleanup};
use silex_dom::view::{AnyView, IntoAnyView, View};
use silex_html::div;
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

/// 能够转换为路由路径的类型
///
/// 用于 `Link` 和 `navigator.push` 等 API，使其同时支持字符串路径和类型安全路由枚举。
pub trait ToRoute {
    fn to_route(&self) -> String;
}

impl ToRoute for &str {
    fn to_route(&self) -> String {
        self.to_string()
    }
}

impl ToRoute for String {
    fn to_route(&self) -> String {
        self.clone()
    }
}

impl ToRoute for &String {
    fn to_route(&self) -> String {
        self.to_string()
    }
}

impl<R: Routable> ToRoute for R {
    fn to_route(&self) -> String {
        self.to_path()
    }
}

/// 路由器组件
#[derive(Clone)]
pub struct Router {
    base_path: String,
    child: Option<Rc<dyn Fn() -> AnyView>>,
}

impl Router {
    /// 创建一个新的 Router
    pub fn new() -> Self {
        Self {
            base_path: "/".to_string(),
            child: None,
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

    /// 设置需要渲染的子视图
    pub fn render<F, V>(mut self, view_fn: F) -> Self
    where
        V: View + Clone + 'static,
        F: Fn() -> V + 'static,
    {
        self.child = Some(Rc::new(move || view_fn().into_any()));
        self
    }

    /// 使用实现了 Routable 的 Enum 进行强类型路由匹配
    pub fn match_enum<R, F, V>(mut self, render: F) -> Self
    where
        R: Routable,
        F: Fn(R) -> V + 'static,
        V: View + Clone + 'static,
    {
        // 创建一个闭包，它在渲染时会获取当前路径并进行匹配
        self.child = Some(Rc::new(move || {
            // 获取当前路径 (这是一个 Signal，所以路径变化时会触发重新渲染)
            let path_signal = crate::router::use_location_path();
            let path = path_signal.get();

            if let Some(matched) = R::match_path(&path) {
                render(matched).into_any()
            } else {
                // 如果没有匹配，渲染空。
                // 用户可以在 Enum 中定义 Fallback 变体 (e.g. #[route("/*")] NotFound) 来处理 404
                AnyView::new(())
            }
        }));
        self
    }

    /// 自动匹配并渲染实现了 RouteView 的路由枚举
    pub fn match_route<R>(mut self) -> Self
    where
        R: RouteView,
    {
        self.child = Some(Rc::new(move || {
            let path_signal = crate::router::use_location_path();
            let path = path_signal.get();

            if let Some(matched) = R::match_path(&path) {
                matched.render()
            } else {
                AnyView::new(())
            }
        }));
        self
    }
}

/// 路由视图特征
///
/// 扩展 Routable，定义了路由如何渲染为视图。
pub trait RouteView: Routable {
    fn render(&self) -> AnyView;
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

        // 3. 提供 Context
        provide_router_context(RouterContextProps {
            base_path: base_path.clone(),
            path,
            search,
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
        let container = div(());
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

        // 7. 渲染 Child
        if let Some(child_factory) = self.child {
            let parent = container_node.clone();
            let factory = child_factory.clone();

            create_effect(move || {
                // 清空容器，准备渲染新的视图
                parent.set_text_content(Some(""));

                // 执行工厂函数获取 View
                // 如果 factory 内部访问了 Signal (如 path)，这个 Effect 会自动建立依赖并在变化时重新运行
                let view = factory();
                view.mount(&parent);
            });
        }
    }
}
