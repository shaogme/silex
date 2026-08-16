#![cfg(target_arch = "wasm32")]

use js_sys::Promise;
use silex::components::Portal;
use silex::flow::Show;
use silex::prelude::*;
use silex_dom::document;
use silex_dom::view::{MountOwnerToken, View};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::Element;

wasm_bindgen_test_configure!(run_in_browser);

async fn flush_browser_tasks() {
    for _ in 0..4 {
        JsFuture::from(Promise::resolve(&JsValue::UNDEFINED))
            .await
            .expect("browser task should resolve");
    }
}

fn host() -> Element {
    let host = document()
        .create_element("div")
        .expect("test host should be creatable");
    document()
        .body()
        .expect("document body should exist")
        .append_child(&host)
        .expect("test host should be attached");
    host
}

fn text_occurrence_count(selector: &str, expected: &str) -> u32 {
    let nodes = document()
        .query_selector_all(selector)
        .expect("selector query should succeed");
    (0..nodes.length())
        .filter_map(|index| nodes.item(index).and_then(|node| node.text_content()))
        .map(|text| text.matches(expected).count() as u32)
        .sum()
}

#[wasm_bindgen_test(async)]
async fn portal_modal_does_not_duplicate_content_after_repeated_toggles() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    let (errors, completed, valid) = root.with_access(|owner| {
        let (show_modal, set_show_modal) =
            owner.signal(false).expect("modal signal should be created");
        let errors = Rc::new(Cell::new(0));
        let completed = Rc::new(Cell::new(false));
        let valid = Rc::new(Cell::new(true));
        let errors_for_handler = errors.clone();
        let error_handler = owner
            .error_handler(move |_| errors_for_handler.set(errors_for_handler.get() + 1))
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = div![
            button("Toggle Modal"),
            Show(ctx, show_modal)
                .children(
                    Portal(
                        ctx,
                        div![
                            h4("I am a Modal!"),
                            p("I am rendered via Portal directly into the body, but I share ctx!")
                        ]
                        .attr("data-test", "portal-modal"),
                    )
                    .build(),
                )
                .build(),
        ];
        let mount_owner = MountOwnerToken::new(owner);
        let _ = view
            .mount(
                &mount_owner,
                host.as_ref(),
                Vec::new(),
                error_handler.view(),
            )
            .expect("portal demo should mount");
        let completed_for_task = completed.clone();
        let valid_for_task = valid.clone();
        owner
            .spawn_scoped(
                async move {
                    for _ in 0..3 {
                        set_show_modal.set(true).expect("modal should open");
                        flush_browser_tasks().await;
                        valid_for_task.set(
                            valid_for_task.get()
                                && text_occurrence_count("body h4", "I am a Modal!") == 1
                                && text_occurrence_count(
                                    "body p",
                                    "I am rendered via Portal directly into the body, but I share ctx!",
                                ) == 1,
                        );
                        set_show_modal.set(false).expect("modal should close");
                        flush_browser_tasks().await;
                        valid_for_task.set(
                            valid_for_task.get()
                                && text_occurrence_count("body h4", "I am a Modal!") == 0
                                && text_occurrence_count(
                                    "body p",
                                    "I am rendered via Portal directly into the body, but I share ctx!",
                                ) == 0,
                        );
                    }
                    completed_for_task.set(true);
                },
                error_handler.view(),
            )
            .expect("toggle task should register");
        (errors, completed, valid)
    });

    for _ in 0..12 {
        flush_browser_tasks().await;
        if completed.get() {
            break;
        }
    }
    assert!(completed.get(), "toggle task should complete");
    assert!(
        valid.get(),
        "portal content should not duplicate across toggles"
    );
    assert_eq!(text_occurrence_count("body h4", "I am a Modal!"), 0);
    assert_eq!(
        text_occurrence_count(
            "body p",
            "I am rendered via Portal directly into the body, but I share ctx!"
        ),
        0
    );
    assert_eq!(errors.get(), 0);

    root.close().expect("root cleanup should succeed");
    assert!(
        host.first_child().is_none(),
        "host should be empty after root cleanup: {}",
        host.inner_html()
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}
