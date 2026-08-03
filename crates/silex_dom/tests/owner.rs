#![cfg(target_arch = "wasm32")]

use silex_core::Runtime;
use silex_dom::attribute::PendingAttribute;
use silex_dom::view::{
    AnyView, ApplyAttributes, IndexedLoopView, KeyedLoopView, RootViewOwner, ScopedViewOwner, View,
    ViewOwner, mount_branch_cached, mount_text_node,
};
use std::{cell::Cell, marker::PhantomData, rc::Rc};
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
    ) {
        let cleanups = self.cleanups.clone();
        owner.on_cleanup(Box::new(move || {
            cleanups.set(cleanups.get() + 1);
        }));
        mount_text_node(parent, &self.text);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        self.mount(owner, parent, attrs);
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

#[wasm_bindgen_test]
fn dynamic_render_owner_cleans_children_on_rerun_and_root_dispose() {
    let host = mount_point();
    let cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let mut root = runtime.run(|scope| {
        let (value, set_value) = scope.signal(0i32);
        let cleanups_for_view = cleanups.clone();
        let view = move || {
            let value = value.get();
            AnyView::new(CleanupProbe {
                text: value.to_string(),
                cleanups: cleanups_for_view.clone(),
            })
        };
        let owner = RootViewOwner::new(scope.clone());
        view.mount_owned(&owner, &host, Vec::new());
        assert_eq!(host.text_content().as_deref(), Some("0"));

        set_value.set(1);
        assert_eq!(host.text_content().as_deref(), Some("1"));
        assert_eq!(cleanups.get(), 1);
    });

    root.dispose().expect("root cleanup should succeed");
    assert_eq!(cleanups.get(), 2);
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}

#[wasm_bindgen_test]
fn branch_replaces_row_owner_and_keyed_list_reorders_ranges() {
    let host = mount_point();
    let branch_cleanups = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let mut root = runtime.run(|scope| {
        let (key, set_key) = scope.signal(0i32);
        let owner = RootViewOwner::new(scope.clone());
        let branch_cleanups_for_view = branch_cleanups.clone();
        mount_branch_cached(
            &owner,
            &host,
            Vec::new(),
            move || key.get(),
            move |key| {
                AnyView::new(CleanupProbe {
                    text: format!("b{key}"),
                    cleanups: branch_cleanups_for_view.clone(),
                })
            },
        );
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
                view_fn: Rc::new(|item: i32, index| format!("{item}:{index};").into_any()),
                error: silex_core::traits::ForErrorHandler::from(move |_| {
                    duplicate_errors_for_handler.set(duplicate_errors_for_handler.get() + 1);
                }),
                _marker: PhantomData,
            };
            let list_owner = ScopedViewOwner::new(child);
            list.mount_owned(&list_owner, &host, Vec::new());
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
    });

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
    let mut root = runtime.run(|scope| {
        scope.child(|child| {
            let (items, set_items) = child.signal(vec![1i32, 2]);
            let list = IndexedLoopView {
                each: items,
                view_fn: Rc::new(|item: i32, index| format!("{item}:{index};").into_any()),
                _marker: PhantomData,
            };
            let owner = ScopedViewOwner::new(child);
            list.mount_owned(&owner, &host, Vec::new());
            assert_eq!(host.text_content().as_deref(), Some("1:0;2:1;"));

            set_items.set(vec![3, 4, 5]);
            assert_eq!(host.text_content().as_deref(), Some("3:0;4:1;5:2;"));
            set_items.set(vec![9]);
            assert_eq!(host.text_content().as_deref(), Some("9:0;"));
        });
    });

    root.dispose().expect("root cleanup should succeed");
    assert!(host.first_child().is_none());
    host.parent_node()
        .expect("test host has a body parent")
        .remove_child(&host)
        .expect("test host can be removed");
}
