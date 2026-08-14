#![cfg(all(target_arch = "wasm32", feature = "page-controller"))]

use silex_bootstrap::{
    AppHostError, BootstrapError, HostState, LifecycleReporter, PageController,
    PageLifecyclePolicy, UnmountOutcome,
};
use silex_core::{Runtime, SilexError, SilexResult};
use silex_dom::{CleanupSink, MountContext, element::Element};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen_test::*;
use web_sys::Node;

wasm_bindgen_test_configure!(run_in_browser);

fn document() -> web_sys::Document {
    web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available")
}

fn target() -> Node {
    let document = document();
    let target: Node = document
        .create_element("div")
        .expect("target can be created")
        .into();
    document
        .body()
        .expect("body is available")
        .append_child(&target)
        .expect("target can be appended");
    target
}

fn detach(target: &Node) {
    if let Some(parent) = target.parent_node() {
        parent.remove_child(target).expect("target can be detached");
    }
}

fn clean_sink() -> CleanupSink {
    CleanupSink::new(|report| assert!(report.is_clean()))
}

fn mount_text<'scope>(ctx: &MountContext<'scope>, text: &'static str) -> SilexResult<()> {
    let handler = ctx.scope().error_handler(|_: SilexError| {})?;
    ctx.mount(Element::with_child("section", text), handler)
}

fn dispatch(event_name: &str) {
    let event = web_sys::Event::new(event_name).expect("event can be created");
    web_sys::window()
        .expect("window is available")
        .dispatch_event(&event)
        .expect("event can be dispatched");
}

fn reporter(calls: Rc<Cell<u32>>) -> LifecycleReporter {
    Rc::new(move |_error| calls.set(calls.get() + 1))
}

#[wasm_bindgen_test]
fn manual_policy_does_not_install_a_listener() {
    let target = target();
    let mut controller = PageController::new(target.clone(), clean_sink());
    controller
        .mount(Runtime::new(), |ctx| mount_text(ctx, "manual"))
        .expect("mount should succeed");
    let calls = Rc::new(Cell::new(0));

    controller
        .install_page_lifecycle(PageLifecyclePolicy::Manual, reporter(calls.clone()))
        .expect("manual policy should succeed");
    dispatch("pagehide");

    assert!(controller.is_active());
    assert_eq!(calls.get(), 0);
    controller.unmount().expect("manual unmount should succeed");
    detach(&target);
}

#[wasm_bindgen_test]
fn pagehide_unmounts_once_and_repeated_events_are_idempotent() {
    let target = target();
    let mut controller = PageController::new(target.clone(), clean_sink());
    controller
        .mount(Runtime::new(), |ctx| mount_text(ctx, "page"))
        .expect("mount should succeed");
    let calls = Rc::new(Cell::new(0));
    controller
        .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter(calls.clone()))
        .expect("pagehide policy should succeed");

    dispatch("pagehide");
    dispatch("pagehide");

    assert!(!controller.is_active());
    assert_eq!(controller.state(), HostState::Ready);
    assert_eq!(target.child_nodes().length(), 0);
    assert_eq!(calls.get(), 0);
    assert_eq!(
        controller
            .unmount()
            .expect("repeated unmount should succeed"),
        UnmountOutcome::AlreadyUnmounted
    );
    detach(&target);
}

#[wasm_bindgen_test]
fn removing_page_lifecycle_keeps_the_application_mounted() {
    let target = target();
    let mut controller = PageController::new(target.clone(), clean_sink());
    controller
        .mount(Runtime::new(), |ctx| mount_text(ctx, "kept"))
        .expect("mount should succeed");
    let calls = Rc::new(Cell::new(0));
    controller
        .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter(calls.clone()))
        .expect("pagehide policy should succeed");
    controller.remove_page_lifecycle();

    dispatch("pagehide");

    assert!(controller.is_active());
    assert_eq!(calls.get(), 0);
    controller
        .unmount()
        .expect("explicit unmount should succeed");
    detach(&target);
}

#[wasm_bindgen_test]
fn visibility_policy_ignores_events_while_document_is_visible() {
    let target = target();
    let mut controller = PageController::new(target.clone(), clean_sink());
    controller
        .mount(Runtime::new(), |ctx| mount_text(ctx, "visible"))
        .expect("mount should succeed");
    let calls = Rc::new(Cell::new(0));
    controller
        .install_page_lifecycle(
            PageLifecyclePolicy::PageHideAndVisibilityChange,
            reporter(calls.clone()),
        )
        .expect("visibility policy should succeed");

    if !document().hidden() {
        dispatch("visibilitychange");
        dispatch("pagehide");
        assert!(controller.is_active());
        assert_eq!(calls.get(), 0);
    }

    controller
        .unmount()
        .expect("explicit unmount should succeed");
    detach(&target);
}

#[wasm_bindgen_test]
fn dropping_controller_removes_listener_before_host_cleanup() {
    let target = target();
    let calls = Rc::new(Cell::new(0));
    let reporter = reporter(calls.clone());

    {
        let mut controller = PageController::new(target.clone(), clean_sink());
        controller
            .mount(Runtime::new(), |ctx| mount_text(ctx, "drop"))
            .expect("mount should succeed");
        controller
            .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter.clone())
            .expect("pagehide policy should succeed");
    }
    drop(reporter);

    dispatch("pagehide");
    assert_eq!(calls.get(), 0);
    assert_eq!(target.child_nodes().length(), 0);
    detach(&target);
}

#[wasm_bindgen_test]
fn lifecycle_reentrancy_is_reported_without_blocking_outer_unmount() {
    let target = target();
    let mut controller = PageController::new(target.clone(), clean_sink());
    controller
        .mount(Runtime::new(), |ctx| {
            let scope = ctx.scope();
            let handler = scope.error_handler(|_: SilexError| {})?;
            scope.on_cleanup(
                || {
                    dispatch("pagehide");
                    Ok(())
                },
                handler,
            )?;
            ctx.mount(Element::with_child("section", "reentrant"), handler)
        })
        .expect("mount should succeed");
    let errors = Rc::new(RefCell::new(None));
    let errors_for_reporter = errors.clone();
    let reporter: LifecycleReporter = Rc::new(move |error| {
        *errors_for_reporter.borrow_mut() = Some(error);
    });
    controller
        .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter)
        .expect("pagehide policy should succeed");

    controller.unmount().expect("outer unmount should succeed");

    let error = errors
        .borrow_mut()
        .take()
        .expect("reentrant lifecycle error should be reported");
    assert!(matches!(
        error,
        BootstrapError::Host(AppHostError::ReentrantOperation)
    ));
    assert_eq!(controller.state(), HostState::Ready);
    assert_eq!(target.child_nodes().length(), 0);
    controller.remove_page_lifecycle();
    detach(&target);
}

#[wasm_bindgen_test]
#[ignore = "wasm32-unknown-unknown uses panic=abort; run in an unwind test target"]
fn lifecycle_reporter_receives_cleanup_error() {
    let target = target();
    let mut controller = PageController::new(target.clone(), clean_sink());
    controller
        .mount(Runtime::new(), |ctx| {
            let scope = ctx.scope();
            let handler = scope.error_handler(|_: SilexError| {})?;
            scope.on_cleanup(
                || -> SilexResult<()> { panic!("page cleanup failure") },
                handler,
            )?;
            ctx.mount(Element::with_child("section", "cleanup-error"), handler)
        })
        .expect("mount should succeed");
    let errors = Rc::new(RefCell::new(None));
    let errors_for_reporter = errors.clone();
    let reporter: LifecycleReporter = Rc::new(move |error| {
        *errors_for_reporter.borrow_mut() = Some(error);
    });
    controller
        .install_page_lifecycle(PageLifecyclePolicy::PageHide, reporter)
        .expect("pagehide policy should succeed");

    dispatch("pagehide");

    let error = errors
        .borrow_mut()
        .take()
        .expect("cleanup error should be reported");
    assert!(matches!(
        error,
        BootstrapError::Host(AppHostError::Dispose(_))
    ));
    assert_eq!(controller.state(), HostState::Poisoned);
    assert_eq!(target.child_nodes().length(), 0);
    controller.remove_page_lifecycle();
    detach(&target);
}
