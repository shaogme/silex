extern crate self as silex;

pub mod core {
    pub use silex_core::*;
}
pub mod dom {
    pub use silex_dom::*;
}
pub mod macros {
    pub use silex_macros::*;
}
pub mod reexports {
    pub use wasm_bindgen;
    pub use web_sys;
}
pub use crate as router;

pub mod context;
pub mod link;

pub use context::*;
pub use link::*;

use silex_core::reactivity::{Signal, on_cleanup};
use silex_core::traits::{RxGet, RxWrite};
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{AnyView, ApplyAttributes, View};
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

use silex_macros::component;

/// 路由器组件入口
///
/// 使用 `#[component]` 标记，推荐用法：
/// `Router().base("/app").children(move |ctx| ...)`
/// 或 `Router().match_route::<AppRoute>()`
#[component]
pub fn Router(
    #[prop(into)]
    #[chain(default = "/")]
    base: String,
    #[prop(render)]
    #[chain(default = Rc::new(|_| AnyView::Empty))]
    children: Rc<dyn Fn(&RouterContext) -> AnyView>,
) -> impl View {
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
        self.children = Rc::new(move |ctx| RouterRouteView::<R>::new(*ctx).into_any());
        self
    }

    /// 使用实现了 `Routable` 的枚举自定义渲染。
    pub fn match_enum<R, F, V>(mut self, render: F) -> Self
    where
        R: Routable + 'static,
        F: Fn(R, RouterContext) -> V + Clone + 'static,
        V: View + 'static,
    {
        self.children =
            Rc::new(move |ctx| RouterMatchView::<R, F, V>::new(render.clone(), *ctx).into_any());
        self
    }
}

#[derive(Clone)]
pub struct RouterView {
    base_path: String,
    children: Rc<dyn Fn(&RouterContext) -> AnyView>,
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

impl ApplyAttributes for RouterView {}

impl View for RouterView {
    fn mount(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        self.clone().mount_owned(parent, attrs);
    }

    fn mount_owned(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        self.mount_internal(parent, attrs);
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

        // 2. 初始化信号与上下文
        let (path, set_path) = Signal::pair(initial_path);
        let (search, set_search) = Signal::pair(initial_search);

        let ctx = RouterContext::new(RouterContextProps {
            base_path: base_path.clone(),
            path,
            search,
            set_path,
            set_search,
        });

        // 3. 监听 popstate
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

        // 4. 清理
        on_cleanup(move || {
            let w = web_sys::window().unwrap();
            let _ = w.remove_event_listener_with_callback(
                "popstate",
                on_popstate.as_ref().unchecked_ref(),
            );
        });

        // 5. 渲染子视图，显式注入 ctx
        let children_view = (self.children)(&ctx);
        children_view.mount_owned(parent, attrs);
    }
}

#[derive(Clone)]
pub struct RouterRouteView<R> {
    ctx: RouterContext,
    _phantom: PhantomData<R>,
}

impl<R> RouterRouteView<R> {
    pub fn new(ctx: RouterContext) -> Self {
        Self {
            ctx,
            _phantom: PhantomData,
        }
    }
}

impl<R> ApplyAttributes for RouterRouteView<R> {}

impl<R> View for RouterRouteView<R>
where
    R: RouteView + 'static,
{
    fn mount(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        silex_dom::view::mount_branch_cached(
            parent,
            attrs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    matched.render(ctx)
                } else {
                    AnyView::Empty
                }
            },
        );
    }

    fn mount_owned(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        silex_dom::view::mount_branch_cached(
            parent,
            attrs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    matched.render(ctx)
                } else {
                    AnyView::Empty
                }
            },
        );
    }
}

#[derive(Clone)]
pub struct RouterMatchView<R, F, V> {
    render: Rc<F>,
    ctx: RouterContext,
    _phantom: PhantomData<(R, V)>,
}

impl<R, F, V> RouterMatchView<R, F, V> {
    pub fn new(render: F, ctx: RouterContext) -> Self {
        Self {
            render: Rc::new(render),
            ctx,
            _phantom: PhantomData,
        }
    }
}

impl<R, F, V> ApplyAttributes for RouterMatchView<R, F, V> {}

impl<R, F, V> View for RouterMatchView<R, F, V>
where
    R: Routable + 'static,
    F: Fn(R, RouterContext) -> V + Clone + 'static,
    V: View + 'static,
{
    fn mount(&self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>) {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        let render = self.render.clone();
        silex_dom::view::mount_branch_cached(
            parent,
            attrs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    render(matched, ctx).into_any()
                } else {
                    AnyView::Empty
                }
            },
        );
    }

    fn mount_owned(self, parent: &web_sys::Node, attrs: Vec<PendingAttribute>)
    where
        Self: Sized,
    {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        let render = self.render;
        silex_dom::view::mount_branch_cached(
            parent,
            attrs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    render(matched, ctx).into_any()
                } else {
                    AnyView::Empty
                }
            },
        );
    }
}

/// 路由视图特征
///
/// 扩展 Routable，定义了路由如何渲染为视图。显式接收 Copy RouterContext 进行渲染。
pub trait RouteView: Routable {
    fn render(&self, ctx: RouterContext) -> AnyView;
}
