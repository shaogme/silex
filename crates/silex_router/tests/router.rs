#![cfg(target_arch = "wasm32")]

use silex_core::Runtime;
use silex_dom::view::{AnyView, ScopedViewOwner, View};
use silex_router::{
    Link, Routable, RouteView, Router, RouterContext, RouterContextProps, RouterRouteView,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[derive(Clone, PartialEq)]
enum TestRoute {
    Home,
    Users,
}

impl Routable for TestRoute {
    fn match_path(path: &str) -> Option<Self> {
        match path {
            "/" | "/home" => Some(Self::Home),
            "/users" => Some(Self::Users),
            _ => None,
        }
    }

    fn to_path(&self) -> String {
        match self {
            Self::Home => "/home".to_string(),
            Self::Users => "/users".to_string(),
        }
    }
}

impl RouteView for TestRoute {
    fn render<'scope>(&self, _ctx: RouterContext<'scope>) -> AnyView<'scope> {
        match self {
            Self::Home => AnyView::from("home"),
            Self::Users => AnyView::from("users"),
        }
    }
}

fn set_url(path: &str) {
    web_sys::window()
        .expect("window is available")
        .history()
        .expect("history is available")
        .replace_state_with_url(&JsValue::NULL, "", Some(path))
        .expect("test URL can be replaced");
}

fn mount_host() -> web_sys::Node {
    let document = web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available");
    let host: web_sys::Node = document
        .create_element("div")
        .expect("host can be created")
        .into();
    let body: web_sys::Node = document.body().expect("body is available").into();
    body.append_child(&host).expect("host can be attached");
    host
}

#[wasm_bindgen_test]
fn router_navigation_popstate_and_dispose_follow_owner() {
    set_url("/app/users?tab=initial");
    let host = mount_host();
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let navigator = Rc::new(RefCell::new(None));
        let navigator_for_children = navigator.clone();
        let navigator_for_cleanup = navigator.clone();
        let view = Router(scope).base("/app").children(Rc::new(move |ctx| {
            *navigator_for_children.borrow_mut() = Some(ctx.navigator);
            RouterRouteView::<TestRoute>::new(ctx).into_any()
        }));
        let owner = ScopedViewOwner::new(scope);
        view.mount_owned(&owner, &host, Vec::new());

        assert_eq!(host.text_content().as_deref(), Some("users"));
        let navigator = navigator
            .borrow()
            .as_ref()
            .copied()
            .expect("router exposes navigator");

        navigator.push(TestRoute::Home);
        assert_eq!(host.text_content().as_deref(), Some("home"));
        assert_eq!(
            web_sys::window().unwrap().location().pathname().unwrap(),
            "/app/home"
        );

        navigator.replace("/users?tab=replaced");
        assert_eq!(host.text_content().as_deref(), Some("users"));
        assert_eq!(
            web_sys::window().unwrap().location().search().unwrap(),
            "?tab=replaced"
        );

        set_url("/app/home?tab=popstate");
        let event = web_sys::Event::new("popstate").expect("popstate event can be created");
        web_sys::window()
            .unwrap()
            .dispatch_event(&event)
            .expect("popstate can be dispatched");
        assert_eq!(host.text_content().as_deref(), Some("home"));

        navigator_for_cleanup.borrow_mut().take();
    });

    root.dispose().expect("root cleanup should succeed");
    set_url("/app/users");
    let event = web_sys::Event::new("popstate").expect("popstate event can be created");
    web_sys::window()
        .unwrap()
        .dispatch_event(&event)
        .expect("popstate can be dispatched");
    assert_eq!(host.text_content().as_deref(), Some(""));

    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}

#[wasm_bindgen_test]
fn link_active_class_tracks_the_router_path() {
    set_url("/");
    let host = mount_host();
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let (path, set_path) = scope.signal(String::from("/users"));
        let (search, set_search) = scope.signal(String::new());
        let context = RouterContext::new(
            scope,
            RouterContextProps {
                base_path: String::from("/app"),
                path,
                search,
                set_path,
                set_search,
            },
        );
        let link = Link("/users")
            .router_ctx(context)
            .children("users")
            .active_class("active");
        let owner = ScopedViewOwner::new(scope);
        link.mount_owned(&owner, &host, Vec::new());

        let element: web_sys::Element = host
            .first_child()
            .expect("link element is mounted")
            .dyn_into()
            .expect("mounted node is an element");
        assert_eq!(element.class_name(), "active");

        set_path.set(String::from("/other"));
        assert_eq!(element.class_name(), "");
    });

    root.dispose().expect("root cleanup should succeed");
    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}
