#![cfg(target_arch = "wasm32")]

use std::{cell::Cell, rc::Rc};

use silex::components::ErrorBoundary;
use silex_core::{
    EffectPhase, ErrorHandlerInput, ErrorHandlerToken, ErrorReporter, OwnerAccess, ReadSignal,
    Runtime, RxGet, SilexContext, SilexError, SilexErrorKind, SilexResult,
};
use silex_dom::adapters::browser::BrowserDom;
use silex_view::{MountContext, MountInstance, MountOwnerToken, View};
use wasm_bindgen_test::*;

#[path = "error_boundary/support.rs"]
mod support;

use support::{
    assert_no_parent_error, wait_until_condition, wait_until_dom_text, wait_until_owner_closed,
    yield_microtask,
};

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("browser tests have a window")
        .document()
        .expect("browser tests have a document")
}

#[derive(Clone)]
struct InitialFailure;

impl<'scope> View<'scope> for InitialFailure {
    fn mount(&self, _context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "initial child failure".to_string(),
        )))
    }
}

#[derive(Clone)]
struct FallbackFailure;

impl<'scope> View<'scope> for FallbackFailure {
    fn mount(&self, _context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "fallback mount failure".to_string(),
        )))
    }
}

#[derive(Clone)]
struct DeferredFailure<'scope> {
    source: ReadSignal<'scope, bool>,
    cleanup_count: Rc<Cell<usize>>,
    effect_runs: Rc<Cell<usize>>,
    failure_count: Rc<Cell<usize>>,
}

impl<'scope> View<'scope> for DeferredFailure<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let node = context.dom().create_text("child")?;
        context.target().append_node(&node)?;
        let node_for_cleanup = node.clone();
        let dom_for_cleanup = context.dom().clone();
        let cleanup_count = self.cleanup_count.clone();
        context.owner().on_cleanup(
            Box::new(move || {
                cleanup_count.set(cleanup_count.get().saturating_add(1));
                if dom_for_cleanup.parent(&node_for_cleanup)?.is_some() {
                    dom_for_cleanup.remove(&node_for_cleanup)?;
                }
                Ok(())
            }),
            context.error_handler(),
        )?;

        let source = self.source;
        let effect_runs = self.effect_runs.clone();
        let failure_count = self.failure_count.clone();
        context.owner().effect(
            EffectPhase::Normal,
            Box::new(move || {
                effect_runs.set(effect_runs.get().saturating_add(1));
                if source.get()? {
                    failure_count.set(failure_count.get().saturating_add(1));
                    return Err(SilexError::recoverable(SilexErrorKind::Framework(
                        "deferred child failure".to_string(),
                    )));
                }
                Ok(())
            }),
            context.error_handler(),
        )?;
        Ok(MountInstance::from_nodes(vec![node]))
    }
}

#[derive(Clone)]
struct ConstructedHandlerFailure<'scope> {
    handler: ErrorReporter<'scope>,
}

impl<'scope> View<'scope> for ConstructedHandlerFailure<'scope> {
    fn mount(&self, _context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let _ = self
            .handler
            .handle(SilexError::recoverable(SilexErrorKind::Framework(
                "constructed child failure".to_string(),
            )));
        Ok(MountInstance::from_nodes(Vec::new()))
    }
}

#[derive(Clone)]
struct RepeatedHandlerFailure<'scope> {
    handler: ErrorReporter<'scope>,
}

impl<'scope> View<'scope> for RepeatedHandlerFailure<'scope> {
    fn mount(&self, _context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        for message in ["first repeated failure", "second repeated failure"] {
            let _ = self
                .handler
                .handle(SilexError::recoverable(SilexErrorKind::Framework(
                    message.to_string(),
                )));
        }
        Ok(MountInstance::from_nodes(Vec::new()))
    }
}

fn host() -> web_sys::Element {
    document()
        .create_element("div")
        .expect("test host should be creatable")
}

fn test_handler<'scope>(
    owner: OwnerAccess<'scope>,
    errors: Rc<Cell<usize>>,
) -> ErrorHandlerToken<'scope> {
    owner
        .error_handler(move |_| errors.set(errors.get() + 1))
        .expect("test error handler should be registered")
}

fn test_owner<'scope>(
    owner: OwnerAccess<'scope>,
    errors: Rc<Cell<usize>>,
) -> (MountOwnerToken<'scope>, ErrorHandlerToken<'scope>) {
    let error_handler = test_handler(owner, errors);
    (MountOwnerToken::new(owner), error_handler)
}

fn mount_view<'scope, V: View<'scope>>(
    view: &V,
    owner: &MountOwnerToken<'scope>,
    parent: &web_sys::Node,
    error_handler: &ErrorHandlerToken<'scope>,
) -> SilexResult<MountInstance<'scope>> {
    let browser = BrowserDom::new(document());
    let parent = browser.from_web_sys_node(parent.clone())?;
    let context = MountContext::for_parent(
        browser.context(),
        parent,
        owner.clone(),
        error_handler.handler_ref(),
    );
    let instance = context.mount(view)?;
    context.transaction().commit()?;
    Ok(instance)
}

#[wasm_bindgen_test]
fn initial_child_error_switches_to_fallback_without_parent_dispatch() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0usize));
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|owner| {
            let (mount_owner, error_handler) = test_owner(owner, parent_errors.clone());
            let ctx = SilexContext::new(owner, error_handler.view());
            let view = ErrorBoundary(ctx, |_| InitialFailure)
                .fallback(|error| format!("fallback: {error}"))
                .build();

            let _ = mount_view(&view, &mount_owner, host.as_ref(), &error_handler)
                .expect("initial child error should be recovered by the boundary");
            assert_eq!(
                host.text_content().as_deref(),
                Some("fallback: Recoverable: Framework Error: initial child failure")
            );
        })
        .expect("initial error boundary child should mount");

    assert_no_parent_error(&parent_errors, || "boundary=fallback".to_string());
}

#[wasm_bindgen_test(async)]
async fn deferred_child_error_reaches_boundary_and_disposes_child() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0usize));
    let cleanup_count = Rc::new(Cell::new(0usize));
    let effect_runs = Rc::new(Cell::new(0usize));
    let failure_count = Rc::new(Cell::new(0usize));
    let fallback_mounts = Rc::new(Cell::new(0usize));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let failed = owner.signal(false).expect("test signal should be created");
        let (mount_owner, error_handler) = test_owner(owner, parent_errors.clone());
        let child = DeferredFailure {
            source: failed.read_signal(),
            cleanup_count: cleanup_count.clone(),
            effect_runs: effect_runs.clone(),
            failure_count: failure_count.clone(),
        };
        let ctx = SilexContext::new(owner, error_handler.view());
        let fallback_mounts = fallback_mounts.clone();
        let view = ErrorBoundary(ctx, move |_| child.clone())
            .fallback(move |_| {
                fallback_mounts.set(fallback_mounts.get().saturating_add(1));
                "fallback"
            })
            .build();

        let _ = mount_view(&view, &mount_owner, host.as_ref(), &error_handler)
            .expect("child should mount before it fails");
        assert_eq!(host.text_content().as_deref(), Some("child"));
        owner
            .spawn_scoped(
                async move {
                    yield_microtask()
                        .await
                        .expect("failure task microtask should resolve");
                    let _ = failed.set(true);
                },
                error_handler.view(),
            )
            .expect("failure task should register");
    });

    wait_until_dom_text(&host, "fallback", 8, || {
        format!(
            "boundary=unknown generation=unknown parent_errors={} cleanup={} fallback_mounts={} effect_runs={} failures={}",
            parent_errors.get(),
            cleanup_count.get(),
            fallback_mounts.get(),
            effect_runs.get(),
            failure_count.get()
        )
    })
    .await;
    wait_until_owner_closed(
        || cleanup_count.get() == 1,
        8,
        || {
            format!(
                "boundary=unknown generation=unknown dom={:?} cleanup={} fallback_mounts={}",
                host.text_content(),
                cleanup_count.get(),
                fallback_mounts.get(),
            )
        },
    )
    .await;
    assert_eq!(fallback_mounts.get(), 1);
    assert_eq!(effect_runs.get(), 2);
    assert_eq!(failure_count.get(), 1);
    assert_no_parent_error(&parent_errors, || "boundary=fallback".to_string());
    root.close().expect("root cleanup should succeed");
    assert_eq!(host.text_content().as_deref(), Some(""));
}

#[wasm_bindgen_test(async)]
async fn child_factory_handler_reaches_boundary_fallback() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0usize));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (mount_owner, error_handler) = test_owner(owner, parent_errors.clone());
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = ErrorBoundary(ctx, move |child_ctx| ConstructedHandlerFailure {
            handler: child_ctx.error_reporter(),
        })
        .fallback(|error| format!("boundary: {error}"))
        .build();

        let _ = mount_view(&view, &mount_owner, host.as_ref(), &error_handler)
            .expect("child handler failure should be deferred");
    });

    wait_until_dom_text(
        &host,
        "boundary: Recoverable: Framework Error: constructed child failure",
        8,
        || format!("boundary=unknown parent_errors={}", parent_errors.get()),
    )
    .await;
    assert_no_parent_error(&parent_errors, || "boundary=fallback".to_string());
    root.close().expect("root cleanup should succeed");
}

#[wasm_bindgen_test(async)]
async fn repeated_deferred_errors_keep_the_first_generation() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0usize));
    let fallback_mounts = Rc::new(Cell::new(0usize));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (mount_owner, error_handler) = test_owner(owner, parent_errors.clone());
        let ctx = SilexContext::new(owner, error_handler.view());
        let fallback_mounts = fallback_mounts.clone();
        let view = ErrorBoundary(ctx, move |child_ctx| RepeatedHandlerFailure {
            handler: child_ctx.error_reporter(),
        })
        .fallback(move |error| {
            fallback_mounts.set(fallback_mounts.get().saturating_add(1));
            format!("first: {error}")
        })
        .build();

        let _ = mount_view(&view, &mount_owner, host.as_ref(), &error_handler)
            .expect("repeated child errors should be deferred");
    });

    wait_until_dom_text(
        &host,
        "first: Recoverable: Framework Error: first repeated failure",
        8,
        || {
            format!(
                "parent_errors={} fallback_mounts={} dom={:?}",
                parent_errors.get(),
                fallback_mounts.get(),
                host.text_content()
            )
        },
    )
    .await;
    assert_eq!(fallback_mounts.get(), 1);
    assert_no_parent_error(&parent_errors, || "boundary=repeated".to_string());
    root.close().expect("root cleanup should succeed");
}

#[wasm_bindgen_test(async)]
async fn fallback_mount_error_reaches_parent_handler() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0usize));
    let fallback_mounts = Rc::new(Cell::new(0usize));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (mount_owner, error_handler) = test_owner(owner, parent_errors.clone());
        let ctx = SilexContext::new(owner, error_handler.view());
        let fallback_mounts = fallback_mounts.clone();
        let view = ErrorBoundary(ctx, move |child_ctx| ConstructedHandlerFailure {
            handler: child_ctx.error_reporter(),
        })
        .fallback(move |_| {
            fallback_mounts.set(fallback_mounts.get().saturating_add(1));
            FallbackFailure
        })
        .build();

        let _ = mount_view(&view, &mount_owner, host.as_ref(), &error_handler)
            .expect("fallback failure should be reported asynchronously");
    });

    wait_until_condition(
        || parent_errors.get() == 1,
        8,
        "the parent fallback error handler",
        || {
            format!(
                "parent_errors={} fallback_mounts={} dom={:?}",
                parent_errors.get(),
                fallback_mounts.get(),
                host.text_content()
            )
        },
    )
    .await;
    assert_eq!(fallback_mounts.get(), 1);
    assert_eq!(host.text_content().as_deref(), Some(""));
    root.close().expect("root cleanup should succeed");
}

#[wasm_bindgen_test(async)]
async fn root_close_during_pending_error_does_not_mount_fallback() {
    let host = host();
    let parent_errors = Rc::new(Cell::new(0usize));
    let fallback_mounts = Rc::new(Cell::new(0usize));
    let cleanup_count = Rc::new(Cell::new(0usize));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let failed = owner.signal(false).expect("test signal should be created");
        let (mount_owner, error_handler) = test_owner(owner, parent_errors.clone());
        let child = DeferredFailure {
            source: failed.read_signal(),
            cleanup_count: cleanup_count.clone(),
            effect_runs: Rc::new(Cell::new(0usize)),
            failure_count: Rc::new(Cell::new(0usize)),
        };
        let ctx = SilexContext::new(owner, error_handler.view());
        let fallback_mounts = fallback_mounts.clone();
        let view = ErrorBoundary(ctx, move |_| child.clone())
            .fallback(move |_| {
                fallback_mounts.set(fallback_mounts.get().saturating_add(1));
                "fallback"
            })
            .build();

        let _ = mount_view(&view, &mount_owner, host.as_ref(), &error_handler)
            .expect("child should mount before root close");
        owner
            .spawn_scoped(
                async move {
                    yield_microtask()
                        .await
                        .expect("first pending microtask should resolve");
                    yield_microtask()
                        .await
                        .expect("second pending microtask should resolve");
                    let _ = failed.set(true);
                },
                error_handler.view(),
            )
            .expect("pending failure task should register");
    });

    root.close().expect("root cleanup should succeed");
    wait_until_dom_text(&host, "", 8, || {
        format!(
            "boundary=closed generation=unknown parent_errors={} cleanup={} fallback_mounts={}",
            parent_errors.get(),
            cleanup_count.get(),
            fallback_mounts.get()
        )
    })
    .await;
    assert_eq!(fallback_mounts.get(), 0);
    assert_no_parent_error(&parent_errors, || "boundary=closed".to_string());
}
