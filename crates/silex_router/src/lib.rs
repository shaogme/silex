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

use silex_core::{Scope, SilexError, SilexResult, reactivity::runtime_inputs_of};
use silex_dom::attribute::PendingAttribute;
use silex_dom::helpers::window_event_listener_untyped_owned;
use silex_dom::view::{AnyView, ApplyAttributes, View, ViewOwner};
use std::marker::PhantomData;
use std::rc::Rc;

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
/// `Router(scope).base("/app").children(move |ctx| ...)`
/// 或 `Router(scope).match_route::<AppRoute>()`
#[component]
pub fn Router<'scope>(
    scope: Scope<'scope>,
    #[prop(into)]
    #[chain(default = "/")]
    base: String,
    #[prop(render)]
    #[chain(default = Rc::new(|_| AnyView::Empty))]
    children: Rc<dyn Fn(RouterContext<'scope>) -> AnyView<'scope> + 'scope>,
) -> SilexResult<RouterView<'scope>> {
    let window = web_sys::window().expect("no global `window` exists");
    let location = window.location();
    let raw_path = location.pathname().unwrap_or_else(|_| "/".to_string());
    let initial_search = location.search().unwrap_or_default();
    let base_path = context::normalize_base_path(&base);
    let initial_path = context::strip_base_path(&base_path, &raw_path);
    let (path, set_path) = scope.signal(initial_path);
    let (search, set_search) = scope.signal(initial_search);
    let context = RouterContext::try_new(
        scope,
        RouterContextProps {
            base_path,
            path,
            search,
            set_path,
            set_search,
        },
    );

    context.map(|context| RouterView { context, children })
}

impl<'scope> RouterComponent<'scope> {
    /// 使用实现了 `RouteView` 的枚举自动匹配并渲染子视图。
    pub fn match_route<R>(mut self) -> Self
    where
        R: RouteView + 'static,
    {
        self.children = Rc::new(move |ctx| RouterRouteView::<R>::new(ctx).into_any());
        self
    }

    /// 使用实现了 `Routable` 的枚举自定义渲染。
    pub fn match_enum<R, F, V>(mut self, render: F) -> Self
    where
        R: Routable + 'static,
        F: Fn(R, RouterContext<'scope>) -> V + Clone + 'scope,
        V: View<'scope> + 'scope,
    {
        self.children =
            Rc::new(move |ctx| RouterMatchView::<R, F, V>::new(render.clone(), ctx).into_any());
        self
    }
}

#[derive(Clone)]
pub struct RouterView<'scope> {
    context: RouterContext<'scope>,
    children: Rc<dyn Fn(RouterContext<'scope>) -> AnyView<'scope> + 'scope>,
}

impl<'scope> ApplyAttributes<'scope> for RouterView<'scope> {}

impl<'scope> View<'scope> for RouterView<'scope> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        self.clone().mount_owned(owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount_internal(owner, parent, attrs)
    }
}

impl<'scope> RouterView<'scope> {
    fn mount_internal(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        let inputs = self.context.runtime_inputs();
        owner.validate_inputs(&inputs)?;

        let token = owner.token();
        let navigator = self.context.navigator;
        let listener = window_event_listener_untyped_owned(&token, "popstate", move |_| {
            navigator.refresh_location();
        })
        .map_err(SilexError::from)?;

        let children_view = (self.children)(self.context);
        if let Err(error) = children_view.mount_owned(owner, parent, attrs) {
            listener.cancel();
            return Err(error);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct RouterRouteView<'scope, R> {
    ctx: RouterContext<'scope>,
    _phantom: PhantomData<R>,
}

impl<'scope, R> RouterRouteView<'scope, R> {
    pub fn new(ctx: RouterContext<'scope>) -> Self {
        Self {
            ctx,
            _phantom: PhantomData,
        }
    }
}

impl<'scope, R> ApplyAttributes<'scope> for RouterRouteView<'scope, R> {}

impl<'scope, R> View<'scope> for RouterRouteView<'scope, R>
where
    R: RouteView + 'static,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        let inputs = runtime_inputs_of(path_signal);
        silex_dom::view::mount_branch_cached(
            owner,
            parent,
            attrs,
            inputs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    matched.render(ctx)
                } else {
                    AnyView::Empty
                }
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        let inputs = runtime_inputs_of(path_signal);
        silex_dom::view::mount_branch_cached(
            owner,
            parent,
            attrs,
            inputs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    matched.render(ctx)
                } else {
                    AnyView::Empty
                }
            },
        )
    }
}

#[derive(Clone)]
pub struct RouterMatchView<'scope, R, F, V> {
    render: Rc<F>,
    ctx: RouterContext<'scope>,
    _phantom: PhantomData<(R, V)>,
}

impl<'scope, R, F, V> RouterMatchView<'scope, R, F, V> {
    pub fn new(render: F, ctx: RouterContext<'scope>) -> Self {
        Self {
            render: Rc::new(render),
            ctx,
            _phantom: PhantomData,
        }
    }
}

impl<'scope, R, F, V> ApplyAttributes<'scope> for RouterMatchView<'scope, R, F, V> {}

impl<'scope, R, F, V> View<'scope> for RouterMatchView<'scope, R, F, V>
where
    R: Routable + 'static,
    F: Fn(R, RouterContext<'scope>) -> V + Clone + 'scope,
    V: View<'scope> + 'scope,
{
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        let inputs = runtime_inputs_of(path_signal);
        let render = self.render.clone();
        silex_dom::view::mount_branch_cached(
            owner,
            parent,
            attrs,
            inputs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    render(matched, ctx).into_any()
                } else {
                    AnyView::Empty
                }
            },
        )
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let ctx = self.ctx;
        let path_signal = ctx.path;
        let inputs = runtime_inputs_of(path_signal);
        let render = self.render;
        silex_dom::view::mount_branch_cached(
            owner,
            parent,
            attrs,
            inputs,
            move || path_signal.get(),
            move |path| {
                if let Some(matched) = R::match_path(&path) {
                    render(matched, ctx).into_any()
                } else {
                    AnyView::Empty
                }
            },
        )
    }
}

/// 路由视图特征
///
/// 扩展 Routable，定义了路由如何渲染为视图。显式接收 Copy RouterContext 进行渲染。
pub trait RouteView: Routable {
    fn render<'scope>(&self, ctx: RouterContext<'scope>) -> AnyView<'scope>;
}
