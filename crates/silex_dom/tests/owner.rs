#![cfg(target_arch = "wasm32")]

use silex_core::{
    ErrorHandler, ErrorReporter, Runtime, RuntimeInputs, SilexError, SilexResult, runtime_inputs_of,
};
use silex_dom::attribute::{AttrOp, CombinedStyles, PendingAttribute};
use silex_dom::element::Element;
use silex_dom::view::{
    AnyView, ApplyAttributes, IndexedLoopView, KeyedLoopView, RowUpdater, ScopedViewOwner, View,
    ViewOwner, mount_branch_cached, mount_text_node,
};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};
use wasm_bindgen_test::*;
use web_sys::Node;

wasm_bindgen_test_configure!(run_in_browser);

struct CleanupProbe {
    text: String,
    cleanups: Rc<Cell<usize>>,
}

impl<'scope> ApplyAttributes<'scope> for CleanupProbe {}

impl<'scope> View<'scope> for CleanupProbe {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
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
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

struct FailingChild {
    cleanups: Rc<Cell<usize>>,
}

impl<'scope> ApplyAttributes<'scope> for FailingChild {}

impl<'scope> View<'scope> for FailingChild {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(
            Box::new(move || {
                cleanups.set(cleanups.get() + 1);
                Ok(())
            }),
            owner.token().error_handler(),
        )?;
        Err(SilexError::Framework("child mount rejected".to_string()))
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

struct StatefulProbe {
    text: String,
    node: Rc<RefCell<Option<Node>>>,
    mounts: Rc<Cell<usize>>,
    cleanups: Rc<Cell<usize>>,
}

impl<'scope> ApplyAttributes<'scope> for StatefulProbe {}

impl<'scope> View<'scope> for StatefulProbe {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        _attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        self.mounts.set(self.mounts.get() + 1);
        let node: Node = web_sys::window()
            .expect("window is available in browser tests")
            .document()
            .expect("document is available in browser tests")
            .create_text_node(&self.text)
            .into();
        parent
            .append_child(&node)
            .map_err(silex_core::SilexError::from)?;
        *self.node.borrow_mut() = Some(node);

        let node_for_cleanup = self.node.clone();
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(
            Box::new(move || {
                node_for_cleanup.borrow_mut().take();
                cleanups.set(cleanups.get() + 1);
                Ok(())
            }),
            owner.token().error_handler(),
        )?;
        Ok(())
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount(owner, parent, attrs)
    }
}

fn mount_point() -> Node {
    let document = web_sys::window()
        .expect("window is available in browser tests")
        .document()
        .expect("document is available in browser tests");
    let host: Node = document
        .create_element("div")
        .expect("test host can be created")
        .into();
    let body: Node = document
        .body()
        .expect("body is available in browser tests")
        .into();
    body.append_child(&host).expect("test host can be mounted");
    host
}

fn comment_count(node: &Node) -> u32 {
    let children = node.child_nodes();
    (0..children.length())
        .filter_map(|index| children.item(index))
        .filter(|child| child.node_type() == 8)
        .count() as u32
}

#[wasm_bindgen_test]
fn native_owner_error_handler_separates_initial_deferred_and_cleanup_errors() {
    let initial_reports = Rc::new(Cell::new(0));
    let initial_reports_for_owner = initial_reports.clone();
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let owner = ScopedViewOwner::with_error_reporter(
            scope,
            ErrorReporter::new(move |_| {
                initial_reports_for_owner.set(initial_reports_for_owner.get() + 1);
            }),
        );
        let result = owner.effect_from(
            RuntimeInputs::new(),
            Box::new(|| Err(SilexError::Framework("initial effect failure".to_string()))),
            owner.token().error_handler(),
        );
        assert!(matches!(
            result,
            Err(SilexError::Framework(message)) if message == "initial effect failure"
        ));
    });
    assert_eq!(initial_reports.get(), 0);

    let deferred_reports = Rc::new(Cell::new(0));
    let cleanup_reports = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let (should_fail, set_should_fail) = scope.signal(false);
        let runs = Rc::new(Cell::new(0));
        let runs_for_effect = runs.clone();
        let deferred_reports_for_owner = deferred_reports.clone();
        let cleanup_reports_for_owner = cleanup_reports.clone();
        let owner = ScopedViewOwner::with_error_reporter(
            scope,
            ErrorReporter::new(move |error| {
                if matches!(&error, SilexError::Framework(message) if message == "deferred effect failure")
                {
                    deferred_reports_for_owner.set(deferred_reports_for_owner.get() + 1);
                }
                if matches!(&error, SilexError::Framework(message) if message == "cleanup failure")
                {
                    cleanup_reports_for_owner.set(cleanup_reports_for_owner.get() + 1);
                }
            }),
        );
        owner
            .effect_from(
                runtime_inputs_of(should_fail),
                Box::new(move || -> SilexResult<()> {
                    if should_fail.try_get().map_err(SilexError::from)? {
                        return Err(SilexError::Framework("deferred effect failure".to_string()));
                    }
                    runs_for_effect.set(runs_for_effect.get() + 1);
                    Ok(())
                }),
                owner.token().error_handler(),
            )
            .expect("initial effect run should succeed");
        assert_eq!(runs.get(), 1);

        owner
            .on_cleanup(
                Box::new(|| Err(SilexError::Framework("cleanup failure".to_string()))),
                owner.token().error_handler(),
            )
            .expect("cleanup registration should succeed");

        set_should_fail.set(true);
        assert_eq!(deferred_reports.get(), 1);
        assert_eq!(runs.get(), 1);
        set_should_fail.set(false);
        assert_eq!(runs.get(), 2);
    }

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(deferred_reports.get(), 1);
    assert_eq!(cleanup_reports.get(), 1);
}

#[wasm_bindgen_test]
fn element_child_failure_rolls_back_provisional_owner_and_dom() {
    let host = mount_point();
    let cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let owner = ScopedViewOwner::new(scope);
        let view = Element::with_child(
            "section",
            FailingChild {
                cleanups: cleanups.clone(),
            },
        );

        assert!(matches!(
            view.mount_owned(&owner, &host, Vec::new()),
            Err(SilexError::Framework(message)) if message == "child mount rejected"
        ));
    });

    assert_eq!(cleanups.get(), 1);
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn keyed_list_initial_key_panic_is_a_mount_error() {
    let host = mount_point();
    let reports = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let (items, _) = scope.signal(vec![1_i32]);
        let reports_for_handler = reports.clone();
        let list = KeyedLoopView {
            each: items,
            key_fn: Rc::new(|_| panic!("key panic")),
            view_fn: Rc::new(|item: i32, _, _| AnyView::new(item.to_string())),
            error_handler: Some(ErrorHandler::new(move |_| {
                reports_for_handler.set(reports_for_handler.get() + 1);
            })),
            _marker: PhantomData,
        };
        let owner = ScopedViewOwner::new(scope);

        assert!(matches!(
            list.mount_owned(&owner, &host, Vec::new()),
            Err(SilexError::Javascript(message)) if message.contains("Keyed list key function")
        ));
    });

    assert_eq!(reports.get(), 0);
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn dynamic_render_owner_cleans_children_on_rerun_and_root_dispose() {
    let host = mount_point();
    let cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let (value, set_value) = scope.signal(0i32);
        let cleanups_for_view = cleanups.clone();
        let view = move || {
            let value = value.get();
            AnyView::new(CleanupProbe {
                text: value.to_string(),
                cleanups: cleanups_for_view.clone(),
            })
        };
        let owner = ScopedViewOwner::new(scope);
        view.mount_owned(&owner, &host, Vec::new())
            .expect("dynamic view should mount");
        assert_eq!(host.text_content().as_deref(), Some("0"));

        set_value.set(1);
        assert_eq!(host.text_content().as_deref(), Some("1"));
        assert_eq!(cleanups.get(), 1);
    }

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(cleanups.get(), 2);
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn combined_reactive_styles_clean_up_properties_on_scope_dispose() {
    let host = mount_point();
    let document = web_sys::window()
        .expect("window is available in browser tests")
        .document()
        .expect("document is available in browser tests");
    let element = document
        .create_element("div")
        .expect("test element can be created");
    host.append_child(&element).expect("element can be mounted");

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (color, set_color) = scope.signal(String::from("red"));
        let owner = ScopedViewOwner::new(scope);
        let token = owner.token();
        let operation = AttrOp::CombinedStyles(CombinedStyles {
            statics: Vec::new(),
            properties: vec![("--dom-owner-color".into(), color.into_rx())],
            sheets: Vec::new(),
        });

        operation
            .apply(&element, &token)
            .expect("combined styles can be applied");
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("--dom-owner-color: red")
        );

        set_color.set(String::from("blue"));
        assert!(
            element
                .get_attribute("style")
                .unwrap_or_default()
                .contains("--dom-owner-color: blue")
        );
    });

    assert!(
        !element
            .get_attribute("style")
            .unwrap_or_default()
            .contains("--dom-owner-color")
    );
    let host_node = host;
    host_node
        .parent_node()
        .expect("test host has a body parent")
        .remove_child(&host_node)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn branch_replaces_row_owner_and_keyed_list_reorders_ranges() {
    let host = mount_point();
    let branch_cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let (key, set_key) = scope.signal(0i32);
        let owner = ScopedViewOwner::new(scope);
        let branch_cleanups_for_view = branch_cleanups.clone();
        let key_inputs = scope.promote(key).runtime_inputs();
        mount_branch_cached(
            &owner,
            &host,
            Vec::new(),
            key_inputs,
            move || key.get(),
            move |key| {
                AnyView::new(CleanupProbe {
                    text: format!("b{key}"),
                    cleanups: branch_cleanups_for_view.clone(),
                })
            },
        )
        .expect("branch should mount");
        assert_eq!(host.text_content().as_deref(), Some("b0"));

        set_key.set(0);
        assert_eq!(branch_cleanups.get(), 1);
        set_key.set(1);
        assert_eq!(host.text_content().as_deref(), Some("b1"));
        assert_eq!(branch_cleanups.get(), 2);

        scope.child(|child| {
            let (items, set_items) = child.signal(vec![1i32, 2, 3]);
            let duplicate_errors = Rc::new(Cell::new(0));
            let duplicate_errors_for_handler = duplicate_errors.clone();
            let list = KeyedLoopView {
                each: items,
                key_fn: Rc::new(|item: &i32| *item),
                view_fn: Rc::new(|item: i32, index, updater| {
                    let node = Rc::new(RefCell::new(None::<Node>));
                    let node_for_update = node.clone();
                    assert!(updater.bind(move |next_item, next_index| {
                        if let Some(node) = node_for_update.borrow().as_ref() {
                            node.set_node_value(Some(&format!("{next_item}:{next_index};")));
                        }
                    }));
                    AnyView::new(StatefulProbe {
                        text: format!("{item}:{index};"),
                        node,
                        mounts: Rc::new(Cell::new(0)),
                        cleanups: Rc::new(Cell::new(0)),
                    })
                }),
                error_handler: Some(ErrorHandler::new(move |_| {
                    duplicate_errors_for_handler.set(duplicate_errors_for_handler.get() + 1);
                })),
                _marker: PhantomData,
            };
            let list_owner = ScopedViewOwner::new(child);
            list.mount_owned(&list_owner, &host, Vec::new())
                .expect("keyed list should mount");
            assert_eq!(host.text_content().as_deref(), Some("b11:0;2:1;3:2;"));

            set_items.set(vec![1, 1]);
            assert_eq!(host.text_content().as_deref(), Some("b11:0;2:1;3:2;"));
            assert_eq!(duplicate_errors.get(), 1);
            set_items.set(vec![3, 1, 2]);
            assert_eq!(host.text_content().as_deref(), Some("b13:0;1:1;2:2;"));
            set_items.set(vec![1]);
            assert_eq!(host.text_content().as_deref(), Some("b11:0;"));
        });
        assert_eq!(host.text_content().as_deref(), Some("b1"));
    }

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(branch_cleanups.get(), 3);
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn indexed_list_preserves_position_identity_across_diff() {
    let host = mount_point();
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        scope.child(|child| {
            let (items, set_items) = child.signal(vec![1i32, 2]);
            let list = IndexedLoopView {
                each: items,
                view_fn: Rc::new(|item: i32, index| format!("{item}:{index};").into_any()),
                _marker: PhantomData,
            };
            let owner = ScopedViewOwner::new(child);
            list.mount_owned(&owner, &host, Vec::new())
                .expect("indexed list should mount");
            assert_eq!(host.text_content().as_deref(), Some("1:0;2:1;"));

            set_items.set(vec![3, 4, 5]);
            assert_eq!(host.text_content().as_deref(), Some("3:0;4:1;5:2;"));
            set_items.set(vec![9]);
            assert_eq!(host.text_content().as_deref(), Some("9:0;"));
        });
    }

    root.dispose().expect("root cleanup should succeed");
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn repeated_branch_and_list_replacement_keeps_owner_lifecycle_stable() {
    let host = mount_point();
    let branch_cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let (key, set_key) = scope.signal(0i32);
        let owner = ScopedViewOwner::new(scope);
        let branch_cleanups_for_view = branch_cleanups.clone();
        mount_branch_cached(
            &owner,
            &host,
            Vec::new(),
            scope.promote(key).runtime_inputs(),
            move || key.get(),
            move |key| {
                AnyView::new(CleanupProbe {
                    text: format!("b{key}"),
                    cleanups: branch_cleanups_for_view.clone(),
                })
            },
        )
        .expect("branch should mount");

        for key in 1..8 {
            set_key.set(key);
            assert_eq!(
                host.text_content().as_deref(),
                Some(format!("b{key}").as_str())
            );
        }

        scope.child(|child| {
            let (items, set_items) = child.signal(vec![0i32]);
            let list = IndexedLoopView {
                each: items,
                view_fn: Rc::new(|item: i32, index| format!("{item}:{index};").into_any()),
                _marker: PhantomData,
            };
            let list_owner = ScopedViewOwner::new(child);
            list.mount_owned(&list_owner, &host, Vec::new())
                .expect("indexed list should mount");

            for values in [vec![1, 2, 3], vec![3], vec![4, 5], vec![6, 7, 8, 9]] {
                set_items.set(values.clone());
                let expected = values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("{value}:{index};"))
                    .fold(String::from("b7"), |mut text, value| {
                        text.push_str(&value);
                        text
                    });
                assert_eq!(host.text_content().as_deref(), Some(expected.as_str()));
            }
        });

        assert_eq!(branch_cleanups.get(), 7);
    }

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(branch_cleanups.get(), 8);
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn stateful_keyed_rows_preserve_mounts_and_invalidate_old_updaters() {
    let host = mount_point();
    let mounts = Rc::new(Cell::new(0));
    let updates = Rc::new(Cell::new(0));
    let cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    let root = runtime.run();
    {
        let scope = root.scope();
        scope.child(|child| {
            let (items, set_items) = child.signal(vec![1i32, 2]);
            let first_updater = Rc::new(RefCell::new(None));
            let mounts_for_factory = mounts.clone();
            let updates_for_factory = updates.clone();
            let cleanups_for_factory = cleanups.clone();
            let first_updater_for_factory = first_updater.clone();
            let view = KeyedLoopView {
                each: items,
                key_fn: Rc::new(|item: &i32| *item),
                view_fn: Rc::new(move |item: i32, index, updater: RowUpdater<'_, i32>| {
                    if item == 1 && first_updater_for_factory.borrow().is_none() {
                        *first_updater_for_factory.borrow_mut() = Some(updater.clone());
                    }
                    let node = Rc::new(RefCell::new(None::<Node>));
                    let node_for_update = node.clone();
                    let updates_for_callback = updates_for_factory.clone();
                    assert!(updater.bind(move |next_item, next_index| {
                        updates_for_callback.set(updates_for_callback.get() + 1);
                        if let Some(node) = node_for_update.borrow().as_ref() {
                            node.set_node_value(Some(&format!("{next_item}:{next_index};")));
                        }
                    }));
                    AnyView::new(StatefulProbe {
                        text: format!("{item}:{index};"),
                        node,
                        mounts: mounts_for_factory.clone(),
                        cleanups: cleanups_for_factory.clone(),
                    })
                }),
                error_handler: Some(ErrorHandler::new(|_| {})),
                _marker: PhantomData,
            };
            let owner = ScopedViewOwner::new(child);
            view.mount_owned(&owner, &host, Vec::new())
                .expect("branch view should mount");
            assert_eq!(host.text_content().as_deref(), Some("1:0;2:1;"));
            assert_eq!(mounts.get(), 2);

            set_items.set(vec![2, 1]);
            assert_eq!(host.text_content().as_deref(), Some("2:0;1:1;"));
            assert_eq!(mounts.get(), 2);
            assert!(updates.get() >= 2);

            let stale = first_updater
                .borrow()
                .as_ref()
                .cloned()
                .expect("first row updater is captured");
            set_items.set(vec![2]);
            assert_eq!(host.text_content().as_deref(), Some("2:0;"));
            assert_eq!(cleanups.get(), 1);
            assert!(!stale.update(9, 0));

            set_items.set(vec![2, 1]);
            assert_eq!(host.text_content().as_deref(), Some("2:0;1:1;"));
            assert_eq!(mounts.get(), 3);
            assert!(!stale.update(9, 0));
        });
        assert!(host.first_child().is_none());
    }

    root.dispose().expect("root cleanup should succeed");
    assert!(host.first_child().is_none());
    assert_eq!(cleanups.get(), 3);
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn rejected_stateful_factory_cleans_uncommitted_row_range() {
    let host = mount_point();
    let cleanups = Rc::new(Cell::new(0));
    let errors = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();

    let root = runtime.run();
    {
        let scope = root.scope();
        scope.child(|child| {
            let (items, set_items) = child.signal(vec![1i32]);
            let cleanups_for_factory = cleanups.clone();
            let errors_for_handler = errors.clone();
            let list = KeyedLoopView {
                each: items,
                key_fn: Rc::new(|item: &i32| *item),
                view_fn: Rc::new(move |item: i32, index, updater: RowUpdater<'_, i32>| {
                    let node = Rc::new(RefCell::new(None::<Node>));
                    let node_for_update = node.clone();
                    if item != 2 {
                        assert!(updater.bind(move |next_item, next_index| {
                            if let Some(node) = node_for_update.borrow().as_ref() {
                                node.set_node_value(Some(&format!("{next_item}:{next_index};")));
                            }
                        }));
                    }
                    AnyView::new(StatefulProbe {
                        text: format!("{item}:{index};"),
                        node,
                        mounts: Rc::new(Cell::new(0)),
                        cleanups: cleanups_for_factory.clone(),
                    })
                }),
                error_handler: Some(ErrorHandler::new(move |_| {
                    errors_for_handler.set(errors_for_handler.get() + 1);
                })),
                _marker: PhantomData,
            };
            let owner = ScopedViewOwner::new(child);
            list.mount_owned(&owner, &host, Vec::new())
                .expect("keyed list should mount");
            assert_eq!(host.text_content().as_deref(), Some("1:0;"));
            assert_eq!(comment_count(&host), 4);

            set_items.set(vec![2]);
            assert_eq!(host.text_content().as_deref(), Some("1:0;"));
            assert_eq!(comment_count(&host), 4);
            assert_eq!(cleanups.get(), 1);
            assert_eq!(errors.get(), 1);

            set_items.set(vec![3]);
            assert_eq!(host.text_content().as_deref(), Some("3:0;"));
            assert_eq!(comment_count(&host), 4);
            assert_eq!(cleanups.get(), 2);
        });
        assert!(host.first_child().is_none());
    }

    root.dispose().expect("root cleanup should succeed");
    assert!(host.first_child().is_none());
    assert_eq!(cleanups.get(), 3);
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}
