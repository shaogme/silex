#![cfg(target_arch = "wasm32")]

use silex_core::{
    ErrorHandlerToken, ErrorReporter, Runtime, SilexError, SilexErrorKind, SilexResult,
};
use silex_dom::attribute::PendingAttribute;
use silex_dom::element::Element;
use silex_dom::mounted::{CleanupOrigin, CleanupSink, MountAvailability, MountedApp};
use silex_dom::view::{AnyView, ApplyAttributes, MountInstance, MountOwner, View, mount_component};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_test::*;
use web_sys::Node;

wasm_bindgen_test_configure!(run_in_browser);

fn host_with_caller_node() -> Node {
    let document = web_sys::window()
        .expect("window is available")
        .document()
        .expect("document is available");
    let body: Node = document.body().expect("body is available").into();
    let host: Node = document
        .create_element("div")
        .expect("host can be created")
        .into();
    let caller_node: Node = document.create_text_node("caller-owned").into();
    host.append_child(&caller_node)
        .expect("caller node can be appended");
    body.append_child(&host).expect("host can be appended");
    host
}

fn error_handler<'owner>(owner: silex_core::OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_: SilexError| {})
        .expect("error handler should register")
}

struct CleanupProbe {
    cleanups: Rc<Cell<usize>>,
}

struct FactoryText {
    created: Rc<Cell<usize>>,
}

impl<'owner> View<'owner> for FactoryText {
    fn mount(
        &self,
        owner: &dyn MountOwner<'owner>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'owner>>,
        error_handler: ErrorReporter<'owner>,
    ) -> SilexResult<MountInstance<'owner>> {
        let index = self.created.get() + 1;
        self.created.set(index);
        let document = web_sys::window()
            .expect("window is available")
            .document()
            .expect("document is available");
        let node: Node = document
            .create_text_node(&format!("factory-{index}"))
            .into();
        parent
            .append_child(&node)
            .map_err(|error| SilexError::fatal(SilexErrorKind::from(error)))?;
        let node_for_cleanup = node.clone();
        owner.on_cleanup(
            Box::new(move || {
                if let Some(parent) = node_for_cleanup.parent_node() {
                    parent
                        .remove_child(&node_for_cleanup)
                        .map_err(|error| SilexError::fatal(SilexErrorKind::from(error)))?;
                }
                Ok(())
            }),
            error_handler,
        )?;
        Ok(MountInstance::from_nodes(vec![node]))
    }
}

#[wasm_bindgen_test]
fn any_view_factory_creates_independent_mount_instances() {
    let host = host_with_caller_node();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root should start");
    {
        let access = root.access();
        let owner = silex_dom::view::MountOwnerToken::new(access);
        let handler = error_handler(access);
        let view = AnyView::new(FactoryText {
            created: Rc::new(Cell::new(0)),
        });

        let first = view
            .mount(&owner, &host, Vec::new(), handler.view())
            .expect("first factory mount should succeed");
        let second = view
            .mount(&owner, &host, Vec::new(), handler.view())
            .expect("second factory mount should succeed");

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert!(
            !first
                .first_node()
                .expect("first instance should have a node")
                .is_same_node(second.first_node())
        );
        assert_eq!(
            host.text_content().as_deref(),
            Some("caller-ownedfactory-1factory-2")
        );
    }

    root.close().expect("factory instances should clean up");
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

struct PanicRollbackView;

impl<'owner> ApplyAttributes<'owner> for PanicRollbackView {}

impl<'owner> View<'owner> for PanicRollbackView {
    fn mount(
        &self,
        owner: &dyn MountOwner<'owner>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'owner>>,
        error_handler: ErrorReporter<'owner>,
    ) -> SilexResult<MountInstance<'owner>> {
        mount_component(
            owner,
            parent,
            attrs,
            error_handler,
            |owner, _, _, handler| {
                owner.on_cleanup(Box::new(|| panic!("provisional cleanup")), handler)?;
                Err(SilexError::recoverable(SilexErrorKind::Framework(
                    "child rejected".to_string(),
                )))
            },
        )
    }
}

impl<'owner> ApplyAttributes<'owner> for CleanupProbe {}

impl<'owner> View<'owner> for CleanupProbe {
    fn mount(
        &self,
        owner: &dyn MountOwner<'owner>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'owner>>,
        error_handler: ErrorReporter<'owner>,
    ) -> SilexResult<MountInstance<'owner>> {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(
            Box::new(move || {
                cleanups.set(cleanups.get() + 1);
                Ok(())
            }),
            error_handler,
        )?;
        let document = web_sys::window()
            .expect("window is available")
            .document()
            .expect("document is available");
        let text: Node = document.create_text_node("mounted").into();
        parent
            .append_child(&text)
            .map_err(|error| SilexError::fatal(SilexErrorKind::from(error)))?;
        Ok(MountInstance::from_nodes(vec![text]))
    }
}

#[wasm_bindgen_test]
fn mounted_app_stages_and_commits_after_the_caller_node() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|report| assert!(report.is_clean())),
    );
    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "app"), handler)
    })
    .expect("mount should commit");

    assert!(app.is_active());
    assert_eq!(app.host(), host);
    assert_eq!(host.text_content().as_deref(), Some("caller-ownedapp"));
    assert_eq!(host.child_nodes().length(), 4);

    drop(app);
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);
    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
fn explicit_dispose_cleans_committed_boundary() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|_| panic!("clean explicit dispose should not report")),
    );
    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "app"), handler)
    })
    .expect("mount should commit");

    app.dispose().expect("explicit dispose should succeed");
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);

    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
fn dispose_after_external_host_removal_is_clean_and_keeps_caller_nodes() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|_| panic!("external removal should be clean")),
    );
    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "app"), handler)
    })
    .expect("mount should commit");

    assert!(app.is_active());
    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed externally");
    assert!(app.is_active());

    app.dispose()
        .expect("detached host should be cleanup-idempotent");
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);
}

#[wasm_bindgen_test]
fn mount_error_rolls_back_staging_without_touching_caller_nodes() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(Runtime::new(), host.clone(), CleanupSink::new(|_| {}));
    let result = app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "partial"), handler)?;
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "primary mount failure".to_string(),
        )))
    });

    let error = match result {
        Ok(_) => panic!("mount should return its primary error"),
        Err(error) => error,
    };
    assert!(matches!(
        error.primary(),
        SilexError::Recoverable(SilexErrorKind::Framework(message)) if message == "primary mount failure"
    ));
    assert!(error.rollback().is_clean());
    assert_eq!(error.availability(), MountAvailability::Retryable);
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);

    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "retry"), handler)
    })
    .expect("clean rollback should allow retrying the same handle");
    assert!(app.is_active());
    assert_eq!(host.text_content().as_deref(), Some("caller-ownedretry"));
    app.dispose().expect("retry should dispose cleanly");

    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
fn mounted_app_remounts_the_same_handle_and_preserves_caller_nodes() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(Runtime::new(), host.clone(), CleanupSink::new(|_| {}));

    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "first"), handler)
    })
    .expect("first mount should commit");
    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "second"), handler)
    })
    .expect("second mount should commit");

    assert!(app.is_active());
    assert!(!app.is_poisoned());
    assert_eq!(app.host(), host);
    assert_eq!(host.text_content().as_deref(), Some("caller-ownedsecond"));
    assert_eq!(host.child_nodes().length(), 4);

    app.dispose().expect("remounted app should dispose cleanly");
    assert!(!app.is_active());
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);

    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
fn disposed_handle_can_mount_again_and_dispose_is_idempotent() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(Runtime::new(), host.clone(), CleanupSink::new(|_| {}));

    app.dispose().expect("ready dispose should be a no-op");
    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "after-ready"), handler)
    })
    .expect("ready handle should mount");
    app.dispose().expect("first dispose should succeed");
    app.dispose().expect("second dispose should be idempotent");

    app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(Element::with_child("section", "after-dispose"), handler)
    })
    .expect("disposed handle should mount again");
    assert_eq!(
        host.text_content().as_deref(),
        Some("caller-ownedafter-dispose")
    );

    app.dispose().expect("final dispose should succeed");
    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
fn root_cleanup_runs_before_boundary_rollback_is_attempted() {
    let host = host_with_caller_node();
    let cleanups = Rc::new(Cell::new(0));
    let mut app = MountedApp::new(Runtime::new(), host.clone(), CleanupSink::new(|_| {}));
    let result = app.mount(|ctx| {
        let owner = ctx.access();
        let handler = error_handler(owner);
        let root_cleanups = cleanups.clone();
        owner
            .on_cleanup(
                move || {
                    root_cleanups.set(root_cleanups.get() + 1);
                    Ok(())
                },
                &handler,
            )
            .expect("root cleanup should register");
        ctx.mount(
            CleanupProbe {
                cleanups: cleanups.clone(),
            },
            &handler,
        )?;
        Err(SilexError::recoverable(SilexErrorKind::Framework(
            "mount rejected".to_string(),
        )))
    });

    let error = match result {
        Ok(_) => panic!("mount should fail"),
        Err(error) => error,
    };
    assert!(error.rollback().is_clean());
    assert_eq!(cleanups.get(), 2);
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);

    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
#[ignore = "wasm32 cannot unwind cleanup panics inside the browser runner"]
fn composite_cleanup_failure_upgrades_primary_and_records_provisional_owner() {
    let host = host_with_caller_node();
    let mut app = MountedApp::new(Runtime::new(), host.clone(), CleanupSink::new(|_| {}));
    let result = app.mount(|ctx| {
        let handler = error_handler(ctx.access());
        ctx.mount(PanicRollbackView, handler)
    });

    let error = result.expect_err("mount should fail during composite rollback");
    assert!(matches!(
        error.primary(),
        SilexError::Fatal(SilexErrorKind::Framework(message)) if message == "child rejected"
    ));
    assert!(
        error
            .rollback()
            .cleanup_failures()
            .iter()
            .any(|failure| failure.origin == CleanupOrigin::ProvisionalOwner)
    );
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}
