pub mod context;
pub mod link;

pub use context::*;
pub use link::*;

use crate::router::context::{RouterContextProps, provide_router_context};
use silex_core::reactivity::{Signal, on_cleanup};
use silex_core::traits::{RxGet, RxWrite};
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{AnyView, ApplyAttributes, Mount, View, MountRef};
use silex_html::div;
use silex_macros::component;
use std::marker::PhantomData;
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

/// 路由器组件入口
///
/// 这是标准组件入口，推荐用法：
///
/// `Router().base("/app").match_route::<AppRoute>()`
#[component(standalone = 0)]
pub fn Router(
    #[prop(into, default = "/")] base: String,
    #[prop(default = AnyView::Empty, render)] children: AnyView,
) -> impl View {
    let base = base.into_owned();
    let children = children.into_owned();
    RouterView {
        base_path: normalize_base_path(&base),
        children,
    }
}

impl RouterComponent {
    /// 使用实现了 `RouteView` 的枚举自动匹配并渲染子视图。
    pub fn match_route<R>(mut self) -> Self
    where
        R: RouteView + 'static,
    {
        self.children = RouterRouteView::<R>::new().into_any();
        self
    }

    /// 使用实现了 `Routable` 的枚举自定义渲染。
    pub fn match_enum<R, F, V>(mut self, render: F) -> Self
    where
        R: Routable + 'static,
        F: Fn(R) -> V + Clone + 'static,
        V: View,
    {
        self.children = RouterMatchView::<R, F, V>::new(render).into_any();
        self
    }
}

fn normalize_base_path(path: &str) -> String {
    let mut p = path.to_string();
    if !p.starts_with('/') {
        p = format!("/{}", p);
    }
    if p.len() > 1 && p.ends_with('/') {
        p.pop();
    }
    p
}

#[derive(Clone)]
struct RouterView {
    base_path: String,
    children: AnyView,
}

impl ApplyAttributes for RouterView {}

impl Mount for RouterView {
    fn mount(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        self.mount_internal(parent, attrs);
    }
}

impl MountRef for RouterView {
    fn mount_ref(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        self.clone().mount_internal(parent, attrs);
    }
}

impl RouterView {
    fn mount_internal(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
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
        let (path, set_path) = Signal::pair(initial_path);
        let (search, set_search) = Signal::pair(initial_search);

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
        container.mount(parent, attrs);

        // 6. 清理
        on_cleanup(move || {
            let w = web_sys::window().unwrap();
            let _ = w.remove_event_listener_with_callback(
                "popstate",
                on_popstate.as_ref().unchecked_ref(),
            );
        });

        // 7. 渲染子视图
        self.children.mount(&container_node, Vec::new());
    }
}

#[derive(Clone)]
struct RouterRouteView<R> {
    _phantom: PhantomData<R>,
}

impl<R> RouterRouteView<R> {
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<R> ApplyAttributes for RouterRouteView<R> {}

impl<R> Mount for RouterRouteView<R>
where
    R: RouteView + 'static,
{
    fn mount(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        let path_signal = crate::router::use_location_path();
        silex_dom::view::mount_branch_cached(
            parent,
            attrs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    matched.render()
                } else {
                    AnyView::Empty
                }
            },
        );
    }
}

impl<R> MountRef for RouterRouteView<R>
where
    R: RouteView + 'static,
{
    fn mount_ref(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        Self::new().mount(parent, attrs);
    }
}

#[derive(Clone)]
struct RouterMatchView<R, F, V> {
    render: Rc<F>,
    _phantom: PhantomData<(R, V)>,
}

impl<R, F, V> RouterMatchView<R, F, V> {
    fn new(render: F) -> Self {
        Self {
            render: Rc::new(render),
            _phantom: PhantomData,
        }
    }
}

impl<R, F, V> ApplyAttributes for RouterMatchView<R, F, V> {}

impl<R, F, V> Mount for RouterMatchView<R, F, V>
where
    R: Routable + 'static,
    F: Fn(R) -> V + Clone + 'static,
    V: View,
{
    fn mount(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        let path_signal = crate::router::use_location_path();
        let render = self.render.clone();
        silex_dom::view::mount_branch_cached(
            parent,
            attrs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    render(matched).into_any()
                } else {
                    AnyView::Empty
                }
            },
        );
    }
}

impl<R, F, V> MountRef for RouterMatchView<R, F, V>
where
    R: Routable + 'static,
    F: Fn(R) -> V + Clone + 'static,
    V: View,
{
    fn mount_ref(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        Self {
            render: self.render.clone(),
            _phantom: PhantomData,
        }
        .mount(parent, attrs);
    }
}

/// 路由视图特征
///
/// 扩展 Routable，定义了路由如何渲染为视图。
pub trait RouteView: Routable {
    fn render(&self) -> AnyView;
}
