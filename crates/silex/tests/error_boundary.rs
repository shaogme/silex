#![cfg(target_arch = "wasm32")]

use std::{cell::Cell, rc::Rc};

use js_sys::Promise;
use silex::components::ErrorBoundary;
use silex_core::{ErrorReporter, ReadSignal, Runtime, SilexError, SilexErrorKind, SilexResult};
use silex_dom::attribute::PendingAttribute;
use silex_dom::document;
use silex_dom::view::{ApplyAttributes, MountInstance, MountOwner, ScopedMountOwner, View};
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
        _owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "initial child failure".to_string(),
        )))
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
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let node = document().create_text_node("child");
        parent
            .append_child(&node)
            .map_err(|error| SilexError::fatal(SilexErrorKind::from(error)))?;
        let node: Node = node.into();
        let node_for_cleanup = node.clone();
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
                if source.get()? {
                    return Err(SilexError::recoverable(SilexErrorKind::Framework(
                        "deferred child failure".to_string(),
                    )));
                }
                Ok(())
            }),
            error_handler,
        )?;
        Ok(MountInstance::from_nodes(vec![node]))
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
        _owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        _error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        let _ = self
            .handler
            .handle(SilexError::recoverable(SilexErrorKind::Framework(
                "constructed child failure".to_string(),
            )));
        Ok(MountInstance::from_nodes(Vec::new()))
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
    scope
        .error_handler(move |_| errors.set(errors.get() + 1))
        .expect("test error handler should be registered")
}

fn test_owner<'scope>(
    scope: silex_core::Scope<'scope>,
    errors: Rc<Cell<usize>>,
) -> (ScopedMountOwner<'scope>, ErrorReporter<'scope>) {
    let error_handler = test_handler(scope, errors);
    (ScopedMountOwner::new(scope), error_handler)
}

#[wasm_bindgen_test]
fn initial_child_error_switches_to_fallback_without_parent_dispatch() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime
        .child(|scope| {
            let (owner, error_handler) = test_owner(scope, parent_errors.clone());
            let view = ErrorBoundary(scope, |_| InitialFailure)
                .error_handler(error_handler)
                .fallback(|error| format!("fallback: {error}"))
                .build();

            let _ = view
                .mount(&owner, host.as_ref(), Vec::new(), error_handler)
                .expect("initial child error should be recovered by the boundary");
            assert_eq!(
                host.text_content().as_deref(),
                Some("fallback: Recoverable: Framework Error: initial child failure")
            );
        })
        .expect("initial error boundary child should mount");

    assert_eq!(parent_errors.get(), 0);
}

#[wasm_bindgen_test(async)]
async fn deferred_child_error_reaches_boundary_and_disposes_child() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root runtime should start");

    let set_failed = root.with_scope(|scope| {
        let (failed, set_failed) = scope.signal(false).expect("test signal should be created");
        let (owner, error_handler) = test_owner(scope, parent_errors.clone());
        let child = DeferredFailure { source: failed };
        let view = ErrorBoundary(scope, move |_| child)
            .error_handler(error_handler)
            .fallback(|_| "fallback")
            .build();

        let _ = view
            .mount(&owner, host.as_ref(), Vec::new(), error_handler)
            .expect("child should mount before it fails");
        assert_eq!(host.text_content().as_deref(), Some("child"));
        set_failed
    });

    set_failed
        .set(true)
        .expect("test signal should be writable");
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
    let root = runtime.run().expect("root runtime should start");

    root.with_scope(|scope| {
        let (owner, error_handler) = test_owner(scope, parent_errors.clone());
        let view = ErrorBoundary(scope, move |child_handler| ConstructedHandlerFailure {
            handler: child_handler,
        })
        .error_handler(error_handler)
        .fallback(|error| format!("boundary: {error}"))
        .build();

        let _ = view
            .mount(&owner, host.as_ref(), Vec::new(), error_handler)
            .expect("child handler failure should be deferred");
    });

    JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
        .await
        .expect("microtask should resolve");

    assert_eq!(
        host.text_content().as_deref(),
        Some("boundary: Recoverable: Framework Error: constructed child failure")
    );
    assert_eq!(parent_errors.get(), 0);
    root.dispose().expect("root cleanup should succeed");
}
