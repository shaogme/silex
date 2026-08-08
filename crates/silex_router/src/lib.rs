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
pub mod path;
pub mod route_table;

pub use context::*;
pub use link::*;
pub use path::*;
pub use route_table::*;

use silex_core::{Scope, SilexResult, reactivity::runtime_inputs_of};
use silex_dom::attribute::PendingAttribute;
use silex_dom::helpers::window_event_listener_untyped_owned;
use silex_dom::view::{AnyView, ApplyAttributes, View, ViewOwner};
use std::rc::Rc;

/// 能够转换为本地路由路径的值。
pub trait ToRoute {
    fn to_route(&self) -> String;
}

impl ToRoute for &str {
    fn to_route(&self) -> String {
        (*self).to_string()
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

impl ToRoute for RoutePath {
    fn to_route(&self) -> String {
        self.as_str().to_string()
    }
}

/// Router 的初始 builder。只有调用 [`RouterBuilder::routes`] 后才会得到可挂载的
/// Router 组件，因此 route table 是必填输入。
pub struct RouterBuilder<'scope> {
    scope: Scope<'scope>,
    base: String,
}

/// 路由器组件入口。
#[allow(non_snake_case)]
pub fn Router<'scope>(scope: Scope<'scope>) -> RouterBuilder<'scope> {
    RouterBuilder {
        scope,
        base: String::from("/"),
    }
}

impl<'scope> RouterBuilder<'scope> {
    /// 设置应用的 base path。
    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// 提供必需的路由表。
    pub fn routes(self, routes: RouteTable<'scope>) -> RouterComponent<'scope> {
        let context = create_router_context(self.scope, &self.base);
        RouterComponent {
            context,
            routes,
            layout: None,
        }
    }
}

fn create_router_context<'scope>(
    scope: Scope<'scope>,
    base: &str,
) -> SilexResult<RouterContext<'scope>> {
    let window = web_sys::window().expect("no global `window` exists");
    let location = window.location();
    let raw_path = location.pathname().unwrap_or_else(|_| "/".to_string());
    let initial_search = location.search().unwrap_or_default();
    let base_path = context::normalize_base_path(base);
    let initial_path = context::strip_base_path(&base_path, &raw_path);
    let (path, set_path) = scope.signal(initial_path);
    let (search, set_search) = scope.signal(initial_search);

    RouterContext::try_new(
        scope,
        RouterContextProps {
            base_path,
            path,
            search,
            set_path,
            set_search,
        },
    )
}

/// Router 的可挂载 builder。route table 已在 [`RouterBuilder::routes`] 中固定。
#[derive(Clone)]
pub struct RouterComponent<'scope> {
    context: SilexResult<RouterContext<'scope>>,
    routes: RouteTable<'scope>,
    layout: Option<RouterLayout<'scope>>,
}

/// Router layout 的类型擦除闭包。
pub type RouterLayout<'scope> =
    Rc<dyn Fn(RouterContext<'scope>, AnyView<'scope>) -> AnyView<'scope> + 'scope>;

impl<'scope> RouterComponent<'scope> {
    /// 设置只创建一次的 layout；outlet 会随当前路径更新。
    pub fn layout<F, V>(mut self, layout: F) -> Self
    where
        F: Fn(RouterContext<'scope>, AnyView<'scope>) -> V + 'scope,
        V: View<'scope> + 'scope,
    {
        self.layout = Some(Rc::new(move |context, outlet| {
            layout(context, outlet).into_any()
        }));
        self
    }
}

impl<'scope> ApplyAttributes<'scope> for RouterComponent<'scope> {}

impl<'scope> View<'scope> for RouterComponent<'scope> {
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
        let context = self.context?;
        RouterView {
            context,
            routes: self.routes,
            layout: self.layout,
        }
        .mount_internal(owner, parent, attrs)
    }
}

/// Router 的实际 view，负责注册 popstate listener 并挂载 layout/outlet。
#[derive(Clone)]
pub struct RouterView<'scope> {
    context: RouterContext<'scope>,
    routes: RouteTable<'scope>,
    layout: Option<RouterLayout<'scope>>,
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
            Ok(())
        })?;

        let outlet = RouteOutlet::new(self.context, self.routes).into_any();
        let view = match self.layout {
            Some(layout) => layout(self.context, outlet),
            None => outlet,
        };
        if let Err(error) = view.mount_owned(owner, parent, attrs) {
            listener.cancel();
            return Err(error);
        }
        Ok(())
    }
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

/// 当前路由对应的动态 outlet。
#[derive(Clone)]
pub struct RouteOutlet<'scope> {
    context: RouterContext<'scope>,
    routes: RouteTable<'scope>,
}

impl<'scope> RouteOutlet<'scope> {
    pub fn new(context: RouterContext<'scope>, routes: RouteTable<'scope>) -> Self {
        Self { context, routes }
    }
}

impl<'scope> ApplyAttributes<'scope> for RouteOutlet<'scope> {}

impl<'scope> View<'scope> for RouteOutlet<'scope> {
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
        let context = self.context;
        let path_signal = context.path;
        let routes = self.routes;
        let inputs = runtime_inputs_of(path_signal);

        silex_dom::view::mount_branch_cached(
            owner,
            parent,
            attrs,
            inputs,
            move || path_signal.get(),
            move |path| routes.resolve(&path, context).unwrap_or(AnyView::Empty),
        )
    }
}
