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

use crate::path::strip_path_prefix;
use crate::route_table::RouteBranchKey;
use silex_core::traits::RxGet;
use silex_core::{Scope, SilexResult, reactivity::runtime_inputs_of};
use silex_dom::attribute::PendingAttribute;
use silex_dom::helpers::window_event_listener_untyped_owned;
use silex_dom::view::{AnyView, ApplyAttributes, View, ViewOwner};
use silex_macros::component;
use std::{cell::RefCell, rc::Rc};

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

/// Router layout 的类型擦除闭包。
pub type RouterLayout<'scope> =
    Rc<dyn Fn(RouterContext<'scope>, AnyView<'scope>) -> AnyView<'scope> + 'scope>;

/// Router layout 的链式输入，默认表示不使用外壳。
#[derive(Clone, Default)]
pub struct RouterLayoutInput<'scope>(Option<RouterLayout<'scope>>);

impl<'scope, F, V> From<F> for RouterLayoutInput<'scope>
where
    F: Fn(RouterContext<'scope>, AnyView<'scope>) -> V + 'scope,
    V: View<'scope> + 'scope,
{
    fn from(layout: F) -> Self {
        Self(Some(Rc::new(move |context, outlet| {
            layout(context, outlet).into_any()
        })))
    }
}

impl<'scope> RouterLayoutInput<'scope> {
    fn into_option(self) -> Option<RouterLayout<'scope>> {
        self.0
    }
}

/// Router 组件。`routes` 是 required chain prop，其余配置由 PropsBuilder 延迟到
/// `.build()` 处理。
#[component]
pub fn Router<'scope>(
    scope: Scope<'scope>,
    #[chain] routes: RouteTable<'scope>,
    #[prop(into)]
    #[chain(default = String::from("/"))]
    base: String,
    #[prop(into)]
    #[chain(default)]
    layout: RouterLayoutInput<'scope>,
) -> SilexResult<RouterView<'scope>> {
    let context = create_router_context(scope, &base)?;
    Ok(RouterView {
        context,
        routes,
        layout: layout.into_option(),
    })
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
    prefix: Option<String>,
}

impl<'scope> RouteOutlet<'scope> {
    pub fn new(context: RouterContext<'scope>, routes: RouteTable<'scope>) -> Self {
        Self {
            context,
            routes,
            prefix: None,
        }
    }

    pub(crate) fn nested(
        context: RouterContext<'scope>,
        routes: RouteTable<'scope>,
        prefix: String,
    ) -> Self {
        Self {
            context,
            routes,
            prefix: Some(prefix),
        }
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
        let routes_for_key = routes.clone();
        let prefix = self.prefix;
        let prefix_for_key = prefix.clone();
        let current_path = Rc::new(RefCell::new(None::<String>));
        let current_path_for_key = current_path.clone();
        let current_path_for_branch = current_path;
        let inputs = runtime_inputs_of(path_signal);

        silex_dom::view::mount_branch_stable_cached(
            owner,
            parent,
            attrs,
            inputs,
            move || {
                let path = path_signal.get();
                *current_path_for_key.borrow_mut() = Some(path.clone());
                nested_outlet_path(prefix_for_key.as_deref(), &path)
                    .map(|path| routes_for_key.branch_key(&path))
                    .unwrap_or(RouteBranchKey::Empty)
            },
            move |_| {
                let path = current_path_for_branch
                    .borrow_mut()
                    .take()
                    .unwrap_or_else(|| path_signal.get_untracked());
                let Some(path) = nested_outlet_path(prefix.as_deref(), &path) else {
                    return AnyView::Empty;
                };
                routes
                    .resolve_branch(&path, context)
                    .map(|(_, view)| view)
                    .unwrap_or(AnyView::Empty)
            },
        )
    }
}

fn nested_outlet_path(prefix: Option<&str>, path: &str) -> Option<String> {
    prefix.map_or_else(
        || {
            Some(if path.is_empty() {
                String::from("/")
            } else {
                path.to_string()
            })
        },
        |prefix| strip_path_prefix(prefix, path),
    )
}
