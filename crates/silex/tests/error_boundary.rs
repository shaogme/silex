#![cfg(target_arch = "wasm32")]

use std::{cell::Cell, rc::Rc};

use js_sys::Promise;
use silex::components::ErrorBoundary;
use silex_core::{ErrorReporter, ReadSignal, Runtime, SilexError, SilexResult};
use silex_dom::attribute::PendingAttribute;
use silex_dom::document;
use silex_dom::view::{ApplyAttributes, ScopedViewOwner, View, ViewOwner};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::Node;

wasm_bindgen_test_configure!(run_in_browser);

#[derive(Clone)]
struct InitialFailure;

impl<'scope> ApplyAttributes<'scope> for InitialFailure {}

impl<'scope> View<'scope> for InitialFailure {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        Err(SilexError::Framework("initial child failure".to_string()))
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

#[derive(Clone, Copy)]
struct DeferredFailure<'scope> {
    source: ReadSignal<'scope, bool>,
}

impl<'scope> ApplyAttributes<'scope> for DeferredFailure<'scope> {}

impl<'scope> View<'scope> for DeferredFailure<'scope> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        let node = document().create_text_node("child");
        parent.append_child(&node)?;
        let node: Node = node.into();
        let node_for_cleanup = node.clone();
        let error_handler = owner.token().error_handler();
        owner.on_cleanup(
            Box::new(move || {
                if let Some(parent) = node_for_cleanup.parent_node() {
                    let _ = parent.remove_child(&node_for_cleanup);
                }
                Ok(())
            }),
            error_handler,
        )?;

        let source = self.source;
        owner.effect_from(
            silex_core::reactivity::runtime_inputs_of(source),
            Box::new(move || {
                if source.try_get()? {
                    return Err(SilexError::Framework("deferred child failure".to_string()));
                }
                Ok(())
            }),
            error_handler,
        )?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

#[derive(Clone)]
struct ConstructedHandlerFailure<'scope> {
    handler: ErrorReporter<'scope>,
}

impl<'scope> ApplyAttributes<'scope> for ConstructedHandlerFailure<'scope> {}

impl<'scope> View<'scope> for ConstructedHandlerFailure<'scope> {
    fn mount(
        &self,
        _owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()> {
        self.handler.handle(SilexError::Framework(
            "constructed child failure".to_string(),
        ));
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

fn host() -> web_sys::Element {
    document()
        .create_element("div")
        .expect("test host should be creatable")
}

fn test_handler<'scope>(
    scope: silex_core::Scope<'scope>,
    errors: Rc<Cell<usize>>,
) -> ErrorReporter<'scope> {
    scope.error_handler(move |_| errors.set(errors.get() + 1))
}

#[wasm_bindgen_test]
fn initial_child_error_switches_to_fallback_without_parent_dispatch() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let owner = ScopedViewOwner::new(scope, test_handler(scope, parent_errors.clone()));
        let view = ErrorBoundary(scope, |_| InitialFailure)
            .fallback(|error| format!("fallback: {error}"))
            .build();

        view.mount_owned(&owner, host.as_ref(), Vec::new())
            .expect("initial child error should be recovered by the boundary");
        assert_eq!(
            host.text_content().as_deref(),
            Some("fallback: Framework Error: initial child failure")
        );
    });

    assert_eq!(parent_errors.get(), 0);
}

#[wasm_bindgen_test(async)]
async fn deferred_child_error_reaches_boundary_and_disposes_child() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    let set_failed = root.with_scope(|scope| {
        let (failed, set_failed) = scope.signal(false);
        let owner = ScopedViewOwner::new(scope, test_handler(scope, parent_errors.clone()));
        let child = DeferredFailure { source: failed };
        let view = ErrorBoundary(scope, move |_| child)
            .fallback(|_| "fallback")
            .build();

        view.mount_owned(&owner, host.as_ref(), Vec::new())
            .expect("child should mount before it fails");
        assert_eq!(host.text_content().as_deref(), Some("child"));
        set_failed
    });

    set_failed.set(true);
    JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
        .await
        .expect("microtask should resolve");

    assert_eq!(host.text_content().as_deref(), Some("fallback"));
    assert_eq!(parent_errors.get(), 0);
    root.dispose().expect("root cleanup should succeed");
    assert_eq!(host.text_content().as_deref(), Some(""));
}

#[wasm_bindgen_test(async)]
async fn child_factory_handler_reaches_boundary_fallback() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();

    root.with_scope(|scope| {
        let owner = ScopedViewOwner::new(scope, test_handler(scope, parent_errors.clone()));
        let view = ErrorBoundary(scope, move |child_handler| ConstructedHandlerFailure {
            handler: child_handler,
        })
        .fallback(|error| format!("boundary: {error}"))
        .build();

        view.mount_owned(&owner, host.as_ref(), Vec::new())
            .expect("child handler failure should be deferred");
    });

    JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
        .await
        .expect("microtask should resolve");

    assert_eq!(
        host.text_content().as_deref(),
        Some("boundary: Framework Error: constructed child failure")
    );
    assert_eq!(parent_errors.get(), 0);
    root.dispose().expect("root cleanup should succeed");
}
