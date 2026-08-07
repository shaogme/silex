#![cfg(target_arch = "wasm32")]

use silex_core::{ErrorReporter, ReadSignal, Runtime, SilexError};
use silex_dom::view::{
    AnyView, ApplyAttributes, ScopedViewOwner, View, ViewOwner, mount_text_node,
};
use silex_router::{
    Link, Routable, RouteView, Router, RouterContext, RouterContextProps, RouterMatchView,
    RouterRouteView, RouterViewFactory,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen(inline_js = r#"
export function installRouterListenerSpy() {
    const spy = {
        adds: 0,
        removes: 0,
        fail: false,
        originalAdd: window.addEventListener,
        originalRemove: window.removeEventListener,
    };

    window.addEventListener = function(name, callback, options) {
        if (name === "popstate") {
            spy.adds += 1;
            if (spy.fail) {
                throw new Error("router listener registration failed");
            }
        }
        return spy.originalAdd.call(this, name, callback, options);
    };
    window.removeEventListener = function(name, callback, options) {
        if (name === "popstate") {
            spy.removes += 1;
        }
        return spy.originalRemove.call(this, name, callback, options);
    };

    return spy;
}

export function setRouterListenerSpyFailure(spy, fail) {
    spy.fail = fail;
}

export function routerListenerSpyCount(spy, name) {
    return name === "add" ? spy.adds : spy.removes;
}

export function restoreRouterListenerSpy(spy) {
    window.addEventListener = spy.originalAdd;
    window.removeEventListener = spy.originalRemove;
}
"#)]
unsafe extern "C" {
    #[wasm_bindgen(js_name = installRouterListenerSpy)]
    fn install_router_listener_spy() -> JsValue;

    #[wasm_bindgen(js_name = setRouterListenerSpyFailure)]
    fn set_router_listener_spy_failure(spy: &JsValue, fail: bool);

    #[wasm_bindgen(js_name = routerListenerSpyCount)]
    fn router_listener_spy_count(spy: &JsValue, name: &str) -> u32;

    #[wasm_bindgen(js_name = restoreRouterListenerSpy)]
    fn restore_router_listener_spy(spy: &JsValue);
}

struct RouterListenerSpy {
    value: JsValue,
}

impl RouterListenerSpy {
    fn new() -> Self {
        Self {
            value: install_router_listener_spy(),
        }
    }

    fn set_failure(&self, fail: bool) {
        set_router_listener_spy_failure(&self.value, fail);
    }

    fn count(&self, name: &str) -> u32 {
        router_listener_spy_count(&self.value, name)
    }
}

impl Drop for RouterListenerSpy {
    fn drop(&mut self) {
        restore_router_listener_spy(&self.value);
    }
}

fn dispatch_popstate() {
    let event = web_sys::Event::new("popstate").expect("popstate event can be created");
    web_sys::window()
        .expect("window is available")
        .dispatch_event(&event)
        .expect("popstate can be dispatched");
}

struct RouterCleanupView {
    text: String,
    cleanups: Rc<Cell<u32>>,
}

impl<'scope> ApplyAttributes<'scope> for RouterCleanupView {}

impl<'scope> View<'scope> for RouterCleanupView {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        _attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(Box::new(move || {
            cleanups.set(cleanups.get() + 1);
        }))?;
        mount_text_node(parent, &self.text)?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

struct FactoryTextView<'scope> {
    text: ReadSignal<'scope, String>,
    cleanups: Rc<Cell<u32>>,
}

impl<'scope> ApplyAttributes<'scope> for FactoryTextView<'scope> {}

impl<'scope> View<'scope> for FactoryTextView<'scope> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        _attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(Box::new(move || {
            cleanups.set(cleanups.get() + 1);
        }))?;
        mount_text_node(parent, &self.text.get())?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

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
    let spy = RouterListenerSpy::new();
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
        view.mount_owned(&owner, &host, Vec::new())
            .expect("router view should mount");

        assert_eq!(host.text_content().as_deref(), Some("users"));
        assert_eq!(spy.count("add"), 1);
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

        let history_length = web_sys::window().unwrap().history().unwrap().length();
        navigator.replace("/users?tab=replaced");
        assert_eq!(host.text_content().as_deref(), Some("users"));
        assert_eq!(
            web_sys::window().unwrap().history().unwrap().length(),
            history_length,
            "replace must reuse the current history entry"
        );
        assert_eq!(
            web_sys::window().unwrap().location().search().unwrap(),
            "?tab=replaced"
        );

        set_url("/app/home?tab=popstate");
        dispatch_popstate();
        assert_eq!(host.text_content().as_deref(), Some("home"));

        navigator_for_cleanup.borrow_mut().take();
    });

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(spy.count("remove"), 1);
    set_url("/app/users");
    dispatch_popstate();
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
        let context = RouterContext::try_new(
            scope,
            RouterContextProps {
                base_path: String::from("/app"),
                path,
                search,
                set_path,
                set_search,
            },
        )
        .expect("router context should be created");
        let link = Link("/users")
            .router_ctx(context)
            .children("users")
            .active_class("active");
        let owner = ScopedViewOwner::new(scope);
        link.mount_owned(&owner, &host, Vec::new())
            .expect("link should mount");

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

#[wasm_bindgen_test]
fn query_memo_handles_empty_multiple_duplicate_delete_and_reactive_changes() {
    set_url("/query?empty&blank=&first=one&second=two&tag=first&tag=last");
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let (path, set_path) = scope.signal(String::from("/query"));
        let (search, set_search) = scope.signal(String::from(
            "?empty&blank=&first=one&second=two&tag=first&tag=last",
        ));
        let context = RouterContext::try_new(
            scope,
            RouterContextProps {
                base_path: String::from("/"),
                path,
                search,
                set_path,
                set_search,
            },
        )
        .expect("router context should be created");
        let query = context.query_map();
        let snapshots = Rc::new(RefCell::new(Vec::<HashMap<String, String>>::new()));
        let snapshots_for_effect = snapshots.clone();
        scope.effect(move || {
            snapshots_for_effect.borrow_mut().push(query.get());
        });

        let initial = query.get();
        assert_eq!(initial.len(), 5);
        assert_eq!(initial.get("empty"), Some(&String::new()));
        assert_eq!(initial.get("blank"), Some(&String::new()));
        assert_eq!(initial.get("first"), Some(&String::from("one")));
        assert_eq!(initial.get("second"), Some(&String::from("two")));
        assert_eq!(initial.get("tag"), Some(&String::from("last")));

        context.navigator.set_query("tag", None);
        let after_delete = query.get();
        assert!(!after_delete.contains_key("tag"));
        assert_eq!(after_delete.get("first"), Some(&String::from("one")));

        set_search.set(String::new());
        assert!(query.get().is_empty());

        set_search.set(String::from("?first=updated&new=value"));
        let updated = query.get();
        assert_eq!(updated.get("first"), Some(&String::from("updated")));
        assert_eq!(updated.get("new"), Some(&String::from("value")));
        assert_eq!(snapshots.borrow().len(), 4);
    });

    root.dispose().expect("root cleanup should succeed");
    set_url("/");
}

#[wasm_bindgen_test]
fn router_lexical_owner_dispose_removes_listener_and_ignores_late_popstate() {
    set_url("/app/users");
    let spy = RouterListenerSpy::new();
    let host = mount_host();
    let cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let cleanups_for_children = cleanups.clone();
        let view = Router(scope).base("/app").children(Rc::new(move |ctx| {
            let cleanups_for_view = cleanups_for_children.clone();
            RouterMatchView::<TestRoute, _, _>::new(
                move |_route, _ctx| {
                    AnyView::new(RouterCleanupView {
                        text: String::from("lexical"),
                        cleanups: cleanups_for_view.clone(),
                    })
                },
                ctx,
            )
            .into_any()
        }));
        let owner = ScopedViewOwner::new(scope);
        view.mount_owned(&owner, &host, Vec::new())
            .expect("router view should mount");

        assert_eq!(host.text_content().as_deref(), Some("lexical"));
        assert_eq!(spy.count("add"), 1);
    });

    assert_eq!(spy.count("remove"), 1);
    assert_eq!(cleanups.get(), 1);
    assert_eq!(host.text_content().as_deref(), Some(""));

    set_url("/app/home");
    dispatch_popstate();
    assert_eq!(host.text_content().as_deref(), Some(""));
    assert_eq!(cleanups.get(), 1);

    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}

#[wasm_bindgen_test]
fn router_stops_before_children_when_listener_registration_fails() {
    set_url("/app/home");
    let spy = RouterListenerSpy::new();
    spy.set_failure(true);
    let host = mount_host();
    let children_calls = Rc::new(Cell::new(0));
    let errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let errors_for_reporter = errors.clone();
        let reporter = ErrorReporter::new(move |_| {
            errors_for_reporter.set(errors_for_reporter.get() + 1);
        });

        let children_calls_for_view = children_calls.clone();
        let view = Router(scope).base("/app").children(Rc::new(move |_ctx| {
            children_calls_for_view.set(children_calls_for_view.get() + 1);
            AnyView::from("must not mount")
        }));
        let owner = ScopedViewOwner::with_error_reporter(scope, reporter);
        assert!(matches!(
            view.mount_owned(&owner, &host, Vec::new()),
            Err(SilexError::Javascript(_))
        ));
    });

    assert_eq!(children_calls.get(), 0);
    assert_eq!(errors.get(), 0);
    assert_eq!(spy.count("add"), 1);
    assert_eq!(spy.count("remove"), 0);
    assert_eq!(host.text_content().as_deref(), Some(""));

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(spy.count("remove"), 0);
    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}

#[wasm_bindgen_test]
fn router_view_factory_mounts_scoped_view_with_dynamic_owner_cleanup() {
    set_url("/");
    let host = mount_host();
    let cleanups = Rc::new(Cell::new(0));
    let factory_calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let (text, set_text) = scope.signal(String::from("factory-one"));
        let cleanups_for_factory = cleanups.clone();
        let calls_for_factory = factory_calls.clone();
        let factory = RouterViewFactory(Rc::new(move || {
            calls_for_factory.set(calls_for_factory.get() + 1);
            AnyView::new(FactoryTextView {
                text,
                cleanups: cleanups_for_factory.clone(),
            })
        }));
        let owner = ScopedViewOwner::new(scope);
        factory
            .mount_owned(&owner, &host, Vec::new())
            .expect("router factory should mount");

        assert_eq!(host.text_content().as_deref(), Some("factory-one"));
        assert_eq!(factory_calls.get(), 1);

        set_text.set(String::from("factory-two"));
        assert_eq!(host.text_content().as_deref(), Some("factory-two"));
        assert_eq!(factory_calls.get(), 2);
        assert_eq!(cleanups.get(), 1);
    });

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(cleanups.get(), 2);
    assert_eq!(host.text_content().as_deref(), Some(""));
    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
}
