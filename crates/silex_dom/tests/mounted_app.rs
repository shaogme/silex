#![cfg(target_arch = "wasm32")]

use silex_core::{ErrorReporter, Runtime, SilexError, SilexResult};
use silex_dom::attribute::PendingAttribute;
use silex_dom::element::Element;
use silex_dom::mounted::{CleanupSink, MountedApp};
use silex_dom::view::{ApplyAttributes, View, ViewOwner};
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

fn error_handler<'scope>(scope: silex_core::Scope<'scope>) -> ErrorReporter<'scope> {
    scope
        .error_handler(|_: SilexError| {})
        .expect("error handler should register")
}

struct CleanupProbe {
    cleanups: Rc<Cell<usize>>,
}

impl<'scope> ApplyAttributes<'scope> for CleanupProbe {}

impl<'scope> View<'scope> for CleanupProbe {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<()> {
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
        parent.append_child(&text).map_err(SilexError::from)?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: ErrorReporter<'scope>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs, error_handler)
    }
}

#[wasm_bindgen_test]
fn mounted_app_stages_and_commits_after_the_caller_node() {
    let host = host_with_caller_node();
    let app = MountedApp::mount(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|report| assert!(report.is_clean())),
        |context| {
            let handler = error_handler(context.scope());
            context.mount(Element::with_child("section", "app"), handler)
        },
    )
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
    let app = MountedApp::mount(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|_| panic!("clean explicit dispose should not report")),
        |context| {
            let handler = error_handler(context.scope());
            context.mount(Element::with_child("section", "app"), handler)
        },
    )
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
    let app = MountedApp::mount(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|_| panic!("external removal should be clean")),
        |context| {
            let handler = error_handler(context.scope());
            context.mount(Element::with_child("section", "app"), handler)
        },
    )
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
    let result = MountedApp::mount(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|_| {}),
        |context| {
            let handler = error_handler(context.scope());
            context.mount(Element::with_child("section", "partial"), handler)?;
            Err(SilexError::Framework("primary mount failure".to_string()))
        },
    );

    let error = match result {
        Ok(_) => panic!("mount should return its primary error"),
        Err(error) => error,
    };
    assert!(matches!(
        error.primary(),
        SilexError::Framework(message) if message == "primary mount failure"
    ));
    assert!(error.rollback().is_clean());
    assert_eq!(host.text_content().as_deref(), Some("caller-owned"));
    assert_eq!(host.child_nodes().length(), 1);

    host.parent_node()
        .expect("host has a body parent")
        .remove_child(&host)
        .expect("host can be removed");
}

#[wasm_bindgen_test]
fn root_cleanup_runs_before_boundary_rollback_is_attempted() {
    let host = host_with_caller_node();
    let cleanups = Rc::new(Cell::new(0));
    let result = MountedApp::mount(
        Runtime::new(),
        host.clone(),
        CleanupSink::new(|_| {}),
        |context| {
            let scope = context.scope();
            let handler = error_handler(scope);
            let root_cleanups = cleanups.clone();
            scope
                .on_cleanup(
                    move || {
                        root_cleanups.set(root_cleanups.get() + 1);
                        Ok(())
                    },
                    handler,
                )
                .expect("root cleanup should register");
            context.mount(
                CleanupProbe {
                    cleanups: cleanups.clone(),
                },
                handler,
            )?;
            Err(SilexError::Framework("mount rejected".to_string()))
        },
    );

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
