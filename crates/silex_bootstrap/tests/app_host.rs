#![cfg(target_arch = "wasm32")]

use silex_bootstrap::{AppHost, AppHostError, HostState, UnmountOutcome};
use silex_core::{Runtime, SilexError, SilexErrorKind, SilexResult};
use silex_dom::{CleanupSink, MountContext, element::Element};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
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

fn mount_text<'scope>(context: &MountContext<'scope>, text: &'static str) -> SilexResult<()> {
    let handler = context.scope().error_handler(|_: SilexError| {})?;
    context.mount(Element::with_child("section", text), handler)
}

fn clean_sink() -> CleanupSink {
    CleanupSink::new(|report| assert!(report.is_clean()))
}

#[wasm_bindgen_test]
fn mount_rejects_active_app_and_unmount_is_idempotent() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    assert_eq!(host.state(), HostState::Ready);
    host.mount(Runtime::new(), |context| mount_text(context, "first"))
        .expect("initial mount should succeed");
    assert_eq!(host.state(), HostState::Active);
    assert!(host.is_active());
    assert_eq!(target.text_content().as_deref(), Some("first"));

    let error = host
        .mount(Runtime::new(), |context| mount_text(context, "second"))
        .expect_err("active host must reject a second mount");
    assert!(matches!(error, AppHostError::AlreadyMounted));
    assert!(host.is_active());
    assert_eq!(target.text_content().as_deref(), Some("first"));

    assert_eq!(
        host.unmount().expect("unmount should succeed"),
        UnmountOutcome::Disposed
    );
    assert_eq!(host.state(), HostState::Ready);
    assert!(!host.is_active());
    assert_eq!(target.text_content().as_deref(), Some(""));
    assert_eq!(
        host.unmount()
            .expect("repeated unmount should be idempotent"),
        UnmountOutcome::AlreadyUnmounted
    );

    detach(&target);
}

#[wasm_bindgen_test]
fn clean_mount_failure_returns_to_ready_and_preserves_primary_error() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    let error = host
        .mount(Runtime::new(), |_context| {
            Err(SilexError::recoverable(SilexErrorKind::Framework(
                "mount rejected".to_string(),
            )))
        })
        .expect_err("builder failure should be returned");

    let mount_error = error
        .mount_error()
        .expect("mount error should be preserved");
    assert!(matches!(
        mount_error.primary(),
        SilexError::Recoverable(SilexErrorKind::Framework(message)) if message == "mount rejected"
    ));
    assert!(mount_error.rollback().is_clean());
    assert_eq!(host.state(), HostState::Ready);
    assert!(!host.is_active());
    assert_eq!(target.child_nodes().length(), 0);

    host.mount(Runtime::new(), |context| mount_text(context, "reused"))
        .expect("clean rollback should leave the host reusable");
    host.unmount().expect("reused app should unmount");
    detach(&target);
}

#[wasm_bindgen_test]
#[ignore = "wasm32-unknown-unknown uses panic=abort; run in an unwind test target"]
fn non_clean_mount_rollback_poisoned_host() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    let error = host
        .mount(Runtime::new(), |context| {
            let scope = context.scope();
            let handler = scope.error_handler(|_: SilexError| {})?;
            scope.on_cleanup(
                || -> SilexResult<()> { panic!("rollback cleanup failure") },
                handler,
            )?;
            Err(SilexError::recoverable(SilexErrorKind::Framework(
                "mount rejected".to_string(),
            )))
        })
        .expect_err("mount should fail");

    let mount_error = error
        .mount_error()
        .expect("mount error should be preserved");
    assert!(!mount_error.rollback().is_clean());
    assert_eq!(host.state(), HostState::Poisoned);
    assert!(!host.is_active());
    assert!(matches!(
        host.mount(Runtime::new(), |context| mount_text(context, "blocked")),
        Err(AppHostError::Poisoned)
    ));

    detach(&target);
}

#[wasm_bindgen_test]
fn replace_disposes_old_app_before_publishing_new_app() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    host.mount(Runtime::new(), |context| mount_text(context, "old"))
        .expect("initial mount should succeed");
    host.replace(Runtime::new(), |context| mount_text(context, "new"))
        .expect("replacement mount should succeed");

    assert_eq!(host.state(), HostState::Active);
    assert!(host.is_active());
    assert_eq!(target.text_content().as_deref(), Some("new"));

    host.unmount().expect("replacement should unmount");
    detach(&target);
}

#[wasm_bindgen_test]
fn separate_hosts_keep_their_apps_and_runtimes_independent() {
    let first_target = target();
    let second_target = target();
    let mut first = AppHost::new(first_target.clone(), clean_sink());
    let mut second = AppHost::new(second_target.clone(), clean_sink());

    first
        .mount(Runtime::new(), |context| mount_text(context, "first"))
        .expect("first app should mount");
    second
        .mount(Runtime::new(), |context| mount_text(context, "second"))
        .expect("second app should mount");

    first.unmount().expect("first app should unmount");
    assert_eq!(second.state(), HostState::Active);
    assert!(second.is_active());
    assert_eq!(second_target.text_content().as_deref(), Some("second"));

    second.unmount().expect("second app should unmount");
    detach(&first_target);
    detach(&second_target);
}

#[wasm_bindgen_test]
fn replace_without_active_app_is_rejected() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    let error = host
        .replace(Runtime::new(), |context| mount_text(context, "new"))
        .expect_err("replace requires an active app");
    assert!(matches!(error, AppHostError::NotMounted));
    assert_eq!(host.state(), HostState::Ready);

    detach(&target);
}

#[wasm_bindgen_test]
#[ignore = "wasm32-unknown-unknown uses panic=abort; run in an unwind test target"]
fn failed_old_dispose_does_not_restore_or_replace_the_old_app() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    host.mount(Runtime::new(), |context| {
        let scope = context.scope();
        let handler = scope.error_handler(|_: SilexError| {})?;
        scope.on_cleanup(
            || -> SilexResult<()> { panic!("old app cleanup failure") },
            handler,
        )?;
        context.mount(Element::with_child("section", "old"), handler)
    })
    .expect("old app should mount");

    let replacement_called = Rc::new(Cell::new(false));
    let replacement_called_by_builder = replacement_called.clone();
    let error = host
        .replace(Runtime::new(), move |_context| {
            replacement_called_by_builder.set(true);
            Ok(())
        })
        .expect_err("failed old disposal should reject replacement");

    assert!(error.dispose_error().is_some());
    assert!(!replacement_called.get());
    assert_eq!(host.state(), HostState::Poisoned);
    assert!(!host.is_active());
    assert_eq!(target.text_content().as_deref(), Some(""));

    detach(&target);
}

#[wasm_bindgen_test]
fn failed_new_mount_leaves_replace_host_empty_and_ready() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    host.mount(Runtime::new(), |context| mount_text(context, "old"))
        .expect("old app should mount");
    let error = host
        .replace(Runtime::new(), |_context| {
            Err(SilexError::recoverable(SilexErrorKind::Framework(
                "new app rejected".to_string(),
            )))
        })
        .expect_err("new mount failure should be returned");

    assert!(error.mount_error().is_some());
    assert_eq!(host.state(), HostState::Ready);
    assert!(!host.is_active());
    assert_eq!(target.child_nodes().length(), 0);

    detach(&target);
}

#[wasm_bindgen_test]
#[ignore = "wasm32-unknown-unknown uses panic=abort; run in an unwind test target"]
fn builder_panic_poisoned_host_before_rethrowing() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());

    let panic = catch_unwind(AssertUnwindSafe(|| {
        host.mount(Runtime::new(), |_context| -> SilexResult<()> {
            panic!("builder panic")
        })
        .expect("builder panic should not return")
    }));

    assert!(panic.is_err());
    assert_eq!(host.state(), HostState::Poisoned);
    assert!(!host.is_active());
    detach(&target);
}

#[wasm_bindgen_test]
fn app_host_drop_delegates_cleanup_once_to_mounted_app() {
    let cleanups = Rc::new(Cell::new(0));
    let target = target();

    {
        let cleanups_by_builder = cleanups.clone();
        let mut host = AppHost::new(target.clone(), clean_sink());
        host.mount(Runtime::new(), move |context| {
            let scope = context.scope();
            let handler = scope.error_handler(|_: SilexError| {})?;
            let cleanups = cleanups_by_builder.clone();
            scope.on_cleanup(
                move || {
                    cleanups.set(cleanups.get() + 1);
                    Ok(())
                },
                handler,
            )?;
            context.mount(Element::with_child("section", "owned"), handler)
        })
        .expect("app should mount");
    }

    assert_eq!(cleanups.get(), 1);
    assert_eq!(target.child_nodes().length(), 0);
    detach(&target);
}

#[wasm_bindgen_test]
fn unmount_after_external_target_removal_still_disposes_owner() {
    let target = target();
    let mut host = AppHost::new(target.clone(), clean_sink());
    host.mount(Runtime::new(), |context| mount_text(context, "detached"))
        .expect("app should mount");

    let parent = target.parent_node().expect("target has a parent");
    parent
        .remove_child(&target)
        .expect("target can be removed externally");
    assert!(host.is_active());
    assert_eq!(
        host.unmount().expect("detached host should unmount"),
        UnmountOutcome::Disposed
    );
    assert_eq!(host.state(), HostState::Ready);
    assert_eq!(target.child_nodes().length(), 0);
}
