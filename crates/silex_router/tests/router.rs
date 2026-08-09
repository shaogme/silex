#![cfg(target_arch = "wasm32")]

use silex_core::{ErrorReporter, ReadSignal, Runtime, SilexError, SilexResult};
use silex_dom::view::{
    AnyView, ApplyAttributes, ScopedViewOwner, View, ViewOwner, mount_text_node,
};
use silex_router::macros::routes;
use silex_router::{
    Link, Navigator, RouteEntry, RoutePath, RouteTable, Router, RouterContext, RouterContextProps,
    RouterViewFactory,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn test_handler<'scope>(scope: silex_core::Scope<'scope>) -> ErrorReporter<'scope> {
    scope.error_handler(|_| {})
}

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

fn navigation_table<'scope>(
    navigator: Rc<RefCell<Option<Navigator<'scope>>>>,
) -> RouteTable<'scope> {
    let home_navigator = navigator.clone();
    let user_navigator = navigator.clone();
    let fallback = RouteEntry::new("/*", move |_, _| Some(AnyView::from("not found")))
        .expect("fallback route should compile");
    RouteTable::from_entries(vec![
        RouteEntry::new("/", move |_, context| {
            *home_navigator.borrow_mut() = Some(context.navigator);
            Some(AnyView::from("home"))
        })
        .expect("home route should compile"),
        RouteEntry::new("/users/:id", move |matched, context| {
            *user_navigator.borrow_mut() = Some(context.navigator);
            let id = matched.parse::<u32>("id").ok()?;
            Some(AnyView::from(id.to_string()))
        })
        .expect("user route should compile"),
        RouteEntry::new("/home", move |_, context| {
            *navigator.borrow_mut() = Some(context.navigator);
            Some(AnyView::from("home"))
        })
        .expect("named home route should compile"),
        fallback,
    ])
    .expect("route table should compile")
}

#[wasm_bindgen_test]
fn router_navigation_uses_required_table_and_updates_outlet() {
    set_url("/app/users/7?tab=initial");
    let spy = RouterListenerSpy::new();
    let host = mount_host();
    let navigator = Rc::new(RefCell::new(None));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let view = Router(scope)
            .base("/app")
            .routes(navigation_table(navigator.clone()));
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        view.mount_owned(&owner, &host, Vec::new())
            .expect("router view should mount");

        assert_eq!(host.text_content().as_deref(), Some("7"));
        assert_eq!(spy.count("add"), 1);
    });

    let navigator = navigator
        .borrow()
        .as_ref()
        .copied()
        .expect("route handler should expose navigator");
    navigator.push(RoutePath::new("/home").expect("route path should be valid"));
    assert_eq!(host.text_content().as_deref(), Some("home"));
    assert_eq!(
        web_sys::window().unwrap().location().pathname().unwrap(),
        "/app/home"
    );

    let history_length = web_sys::window().unwrap().history().unwrap().length();
    navigator.replace("/users/8?tab=replaced");
    assert_eq!(host.text_content().as_deref(), Some("8"));
    assert_eq!(
        web_sys::window().unwrap().history().unwrap().length(),
        history_length,
        "replace must reuse the current history entry"
    );
    assert_eq!(
        web_sys::window().unwrap().location().search().unwrap(),
        "?tab=replaced"
    );

    set_url("/app/users/9?tab=popstate");
    dispatch_popstate();
    assert_eq!(host.text_content().as_deref(), Some("9"));

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(spy.count("remove"), 1);
    set_url("/app/users/10");
    dispatch_popstate();
    assert_eq!(host.text_content().as_deref(), Some(""));

    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}

#[wasm_bindgen_test]
fn router_layout_is_created_once_while_outlet_changes() {
    set_url("/app/one");
    let host = mount_host();
    let navigator = Rc::new(RefCell::new(None));
    let layouts = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let navigator_for_one = navigator.clone();
        let navigator_for_two = navigator.clone();
        let table = RouteTable::from_entries(vec![
            RouteEntry::new("/one", move |_, context| {
                *navigator_for_one.borrow_mut() = Some(context.navigator);
                Some(AnyView::from("one"))
            })
            .expect("first route should compile"),
            RouteEntry::new("/two", move |_, context| {
                *navigator_for_two.borrow_mut() = Some(context.navigator);
                Some(AnyView::from("two"))
            })
            .expect("second route should compile"),
        ])
        .expect("route table should compile");
        let layouts_for_view = layouts.clone();
        let view = Router(scope)
            .base("/app")
            .routes(table)
            .layout(move |_context, outlet| {
                layouts_for_view.set(layouts_for_view.get() + 1);
                outlet
            });
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        view.mount_owned(&owner, &host, Vec::new())
            .expect("router view should mount");
    });

    assert_eq!(host.text_content().as_deref(), Some("one"));
    assert_eq!(layouts.get(), 1);
    let navigator = navigator
        .borrow()
        .as_ref()
        .copied()
        .expect("route handler should expose navigator");
    navigator.push(RoutePath::new("/two").expect("route path should be valid"));
    assert_eq!(host.text_content().as_deref(), Some("two"));
    assert_eq!(layouts.get(), 1);

    root.dispose().expect("root cleanup should succeed");
    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}

#[wasm_bindgen_test]
fn nested_outlet_keeps_parent_layout_while_child_route_changes() {
    set_url("/app/users/1");
    let host = mount_host();
    let layouts = Rc::new(Cell::new(0));
    let navigator = Rc::new(RefCell::new(None));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let navigator_for_detail = navigator.clone();
        let layouts_for_view = layouts.clone();
        let routes = routes!(NestedRoutes {
            home "/" => move |_ctx| AnyView::from("home"),
            nest users "/users" => move |_ctx, outlet| {
                layouts_for_view.set(layouts_for_view.get() + 1);
                AnyView::from(vec![AnyView::from("users:"), outlet])
            } {
                detail "/:id" => move |context, id: u32| {
                    *navigator_for_detail.borrow_mut() = Some(context.navigator);
                    AnyView::from(id.to_string())
                },
            },
        });
        let view = Router(scope).base("/app").routes(routes.table());
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        view.mount_owned(&owner, &host, Vec::new())
            .expect("nested router should mount");
    });

    assert_eq!(host.text_content().as_deref(), Some("users:1"));
    assert_eq!(layouts.get(), 1);
    let navigator = navigator
        .borrow()
        .as_ref()
        .copied()
        .expect("nested route should expose navigator");
    navigator.push(RoutePath::new("/users/2").expect("nested path should be valid"));
    assert_eq!(host.text_content().as_deref(), Some("users:2"));
    assert_eq!(layouts.get(), 1);

    root.dispose().expect("root cleanup should succeed");
    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
}

#[wasm_bindgen_test]
fn link_requires_context_and_tracks_active_path() {
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
        let link = Link(context, RoutePath::new("/users").unwrap())
            .children("users")
            .active_class("active");
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        link.mount_owned(&owner, &host, Vec::new())
            .expect("link should mount");

        let element: web_sys::Element = host
            .first_child()
            .expect("link element is mounted")
            .dyn_into()
            .expect("mounted node is an element");
        assert_eq!(element.get_attribute("href").as_deref(), Some("/app/users"));
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
        scope
            .effect(
                move || -> SilexResult<()> {
                    snapshots_for_effect.borrow_mut().push(query.try_get()?);
                    Ok(())
                },
                test_handler(scope),
            )
            .expect("query effect can be registered");

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
    ) -> SilexResult<()> {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(
            Box::new(move || {
                cleanups.set(cleanups.get() + 1);
                Ok(())
            }),
            owner.token().error_handler(),
        )?;
        mount_text_node(parent, &self.text)?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

#[wasm_bindgen_test]
fn router_owner_dispose_removes_listener_and_ignores_late_popstate() {
    set_url("/app/users");
    let spy = RouterListenerSpy::new();
    let host = mount_host();
    let cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let cleanups_for_route = cleanups.clone();
        let table = RouteTable::from_entries(vec![
            RouteEntry::new("/users", move |_, _| {
                Some(AnyView::new(RouterCleanupView {
                    text: String::from("lexical"),
                    cleanups: cleanups_for_route.clone(),
                }))
            })
            .expect("route should compile"),
        ])
        .expect("route table should compile");
        let view = Router(scope).base("/app").routes(table);
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
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
fn router_does_not_mount_outlet_when_listener_registration_fails() {
    set_url("/app/home");
    let spy = RouterListenerSpy::new();
    spy.set_failure(true);
    let host = mount_host();
    let route_calls = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let calls = route_calls.clone();
        let table = RouteTable::from_entries(vec![
            RouteEntry::new("/home", move |_, _| {
                calls.set(calls.get() + 1);
                Some(AnyView::from("must not mount"))
            })
            .expect("route should compile"),
        ])
        .expect("route table should compile");
        let view = Router(scope).base("/app").routes(table);
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
        assert!(matches!(
            view.mount_owned(&owner, &host, Vec::new()),
            Err(SilexError::Javascript(_))
        ));
    });

    assert_eq!(route_calls.get(), 0);
    assert_eq!(spy.count("add"), 1);
    assert_eq!(spy.count("remove"), 0);
    assert_eq!(host.text_content().as_deref(), Some(""));

    root.dispose().expect("root cleanup should succeed");
    host.parent_node()
        .expect("host has a parent")
        .remove_child(&host)
        .expect("host can be removed");
    set_url("/");
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
    ) -> SilexResult<()> {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(
            Box::new(move || {
                cleanups.set(cleanups.get() + 1);
                Ok(())
            }),
            owner.token().error_handler(),
        )?;
        mount_text_node(parent, &self.text.get())?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<silex_dom::attribute::PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

#[wasm_bindgen_test]
fn router_view_factory_keeps_scoped_dynamic_owner_cleanup() {
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
        let owner = ScopedViewOwner::new(scope, test_handler(scope));
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
