#![cfg(target_arch = "wasm32")]
#![deny(warnings)]

use js_sys::Promise;
use silex::prelude::*;
use silex::ui::{AccordionContent, AccordionContentMode, AccordionItem, AccordionTrigger};
use silex_dom::document;
use silex_dom::view::{MountErrorHandler, MountInstance, MountOwner, MountOwnerToken, View};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::{Element, HtmlElement, HtmlInputElement};

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

fn content(host: &Element) -> Element {
    host.query_selector("[data-slot='accordion-content']")
        .expect("content selector should be valid")
        .expect("content wrapper should be mounted")
}

fn trigger(host: &Element) -> Element {
    host.query_selector("[data-slot='accordion-trigger']")
        .expect("trigger selector should be valid")
        .expect("trigger should be mounted")
}

fn child_input(content: &Element) -> HtmlInputElement {
    content
        .query_selector("[data-test='accordion-input']")
        .expect("input selector should be valid")
        .expect("input should be mounted with the wrapper")
        .dyn_into::<HtmlInputElement>()
        .expect("accordion child should be an input")
}

fn slot_button(content: &Element) -> Element {
    content
        .query_selector("[data-test='accordion-slot-button']")
        .expect("slot button selector should be valid")
        .expect("slot button should be mounted with the slot")
}

fn dispatch_click(element: &Element) {
    let event = web_sys::MouseEvent::new("click").expect("click event should be creatable");
    element
        .dispatch_event(&event)
        .expect("click event should dispatch");
}

#[wasm_bindgen_test(async)]
async fn trigger_and_content_keep_accessibility_state_and_focus_in_sync() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");
    let task_host = host.clone();

    let (completed, valid, errors) = root.with_access(|owner| {
        let (open, set_open) = owner
            .signal(true)
            .expect("accordion open signal should be created");
        let errors = Rc::new(Cell::new(0usize));
        let errors_for_handler = errors.clone();
        let error_handler = owner
            .error_handler(move |_| {
                errors_for_handler.set(errors_for_handler.get().saturating_add(1));
            })
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let set_open_for_click = set_open;
        let on_click = owner
            .callback(move |_| {
                set_open_for_click.update(|value| *value = !*value)?;
                Ok(())
            })
            .expect("accordion trigger callback should be created");
        let view = AccordionItem(
            ctx,
            chain!(
                AccordionTrigger(ctx, "Toggle")
                    .open(open)
                    .expect("trigger open prop should be created")
                    .on_click(on_click)
                    .build()
                    .expect("trigger should build"),
                AccordionContent(ctx, input().attr("data-test", "accordion-input"))
                    .open(open)
                    .expect("content open prop should be created")
                    .build()
                    .expect("content should build")
            ),
            "focus-item",
        )
        .build()
        .expect("accordion item should build");
        let mount_owner = MountOwnerToken::new(owner);
        let _ = view
            .mount(
                &mount_owner,
                host.as_ref(),
                Vec::new(),
                error_handler.view(),
            )
            .expect("accordion item should mount");

        let completed = Rc::new(Cell::new(false));
        let valid = Rc::new(Cell::new(true));
        let completed_for_task = completed.clone();
        let valid_for_task = valid.clone();
        let task_host = task_host.clone();
        owner
            .spawn_scoped(
                async move {
                    flush_browser_tasks().await;
                    let mounted_trigger = trigger(&task_host);
                    let mounted_content = content(&task_host);
                    let trigger_id = mounted_trigger.get_attribute("id").unwrap_or_else(|| {
                        panic!("trigger id should be generated: {}", task_host.inner_html())
                    });
                    let content_id = mounted_content.get_attribute("id").unwrap_or_else(|| {
                        panic!("content id should be generated: {}", task_host.inner_html())
                    });
                    assert!(!trigger_id.is_empty());
                    assert!(!content_id.is_empty());
                    assert_eq!(
                        mounted_trigger.get_attribute("aria-expanded").as_deref(),
                        Some("true")
                    );
                    assert_eq!(
                        mounted_content.get_attribute("aria-hidden").as_deref(),
                        Some("false")
                    );
                    assert!(!mounted_content.has_attribute("inert"));
                    assert_eq!(
                        mounted_trigger.get_attribute("aria-controls").as_deref(),
                        Some(content_id.as_str())
                    );
                    assert_eq!(
                        mounted_content.get_attribute("aria-labelledby").as_deref(),
                        Some(trigger_id.as_str())
                    );

                    let task_trigger = mounted_trigger
                        .dyn_into::<HtmlElement>()
                        .expect("trigger should be focusable");
                    task_trigger.click();
                    flush_browser_tasks().await;
                    let closed_trigger = trigger(&task_host);
                    let closed_content = content(&task_host);
                    let active_after_click = document()
                        .active_element()
                        .expect("document should expose active element");
                    valid_for_task.set(
                        valid_for_task.get()
                            && closed_trigger.get_attribute("aria-expanded").as_deref()
                                == Some("false")
                            && closed_content.get_attribute("aria-hidden").as_deref()
                                == Some("true")
                            && closed_content.has_attribute("inert")
                            && closed_trigger.is_same_node(Some(active_after_click.as_ref())),
                    );

                    set_open.set(true).expect("accordion should reopen");
                    flush_browser_tasks().await;
                    let reopened_content = content(&task_host);
                    let input = child_input(&reopened_content);
                    input.focus().expect("content input should receive focus");
                    let active_input = document()
                        .active_element()
                        .expect("document should expose active element");
                    valid_for_task.set(
                        valid_for_task.get()
                            && input.is_same_node(Some(active_input.as_ref()))
                            && !reopened_content.has_attribute("inert"),
                    );

                    set_open.set(false).expect("external close should succeed");
                    flush_browser_tasks().await;
                    let externally_closed_trigger = trigger(&task_host);
                    let externally_closed_content = content(&task_host);
                    let active_after_external_close = document()
                        .active_element()
                        .expect("document should expose active element");
                    valid_for_task.set(
                        valid_for_task.get()
                            && externally_closed_content.has_attribute("inert")
                            && externally_closed_trigger
                                .is_same_node(Some(active_after_external_close.as_ref()))
                            && !externally_closed_content
                                .contains(Some(active_after_external_close.as_ref())),
                    );

                    let _ = child_input(&externally_closed_content)
                        .focus()
                        .map_err(SilexError::fatal);
                    let active_after_inert_focus = document()
                        .active_element()
                        .expect("document should expose active element");
                    valid_for_task.set(
                        valid_for_task.get()
                            && externally_closed_trigger
                                .is_same_node(Some(active_after_inert_focus.as_ref())),
                    );
                    completed_for_task.set(true);
                },
                error_handler.view(),
            )
            .expect("accordion interaction task should register");
        (completed, valid, errors)
    });

    for _ in 0..12 {
        flush_browser_tasks().await;
        if completed.get() {
            break;
        }
    }
    assert!(
        completed.get(),
        "accordion interaction task should complete"
    );
    assert!(
        valid.get(),
        "accordion trigger/content accessibility and focus state should stay synchronized"
    );
    assert_eq!(
        errors.get(),
        0,
        "accordion should not report runtime errors"
    );

    root.close()
        .expect("accordion owner cleanup should succeed");
    assert!(
        host.first_child().is_none(),
        "accordion host should be empty after cleanup"
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test(async)]
async fn keep_alive_preserves_wrapper_and_child_identity_when_toggled() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");
    let task_host = host.clone();

    let (completed, valid, errors) = root.with_access(|owner| {
        let (open, set_open) = owner
            .signal(false)
            .expect("accordion open signal should be created");
        let errors = Rc::new(Cell::new(0usize));
        let errors_for_handler = errors.clone();
        let error_handler = owner
            .error_handler(move |_| {
                errors_for_handler.set(errors_for_handler.get().saturating_add(1));
            })
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = AccordionContent(
            ctx,
            input()
                .attr("data-test", "accordion-input")
                .attr("value", "initial"),
        )
        .open(open)
        .expect("accordion open prop should be created")
        .build()
        .expect("accordion content should build");
        let mount_owner = MountOwnerToken::new(owner);
        let _ = view
            .mount(
                &mount_owner,
                host.as_ref(),
                Vec::new(),
                error_handler.view(),
            )
            .expect("accordion content should mount");

        let initial_content = content(&host);
        let initial_input = child_input(&initial_content);
        initial_input.set_value("typed");
        assert_eq!(
            initial_content.get_attribute("data-state").as_deref(),
            Some("closed")
        );
        assert_eq!(
            initial_content.get_attribute("aria-hidden").as_deref(),
            Some("true")
        );
        assert!(initial_content.has_attribute("inert"));

        let completed = Rc::new(Cell::new(false));
        let valid = Rc::new(Cell::new(true));
        let completed_for_task = completed.clone();
        let valid_for_task = valid.clone();
        let initial_content_for_task = initial_content.clone();
        let initial_input_for_task = initial_input.clone();
        let task_host = task_host.clone();
        owner
            .spawn_scoped(
                async move {
                    set_open.set(true).expect("accordion should open");
                    flush_browser_tasks().await;
                    let open_content = content(&task_host);
                    let open_input = child_input(&open_content);
                    valid_for_task.set(
                        valid_for_task.get()
                            && initial_content_for_task.is_same_node(Some(open_content.as_ref()))
                            && initial_input_for_task.is_same_node(Some(open_input.as_ref()))
                            && open_input.value() == "typed"
                            && open_content.get_attribute("data-state").as_deref() == Some("open")
                            && open_content.get_attribute("aria-hidden").as_deref()
                                == Some("false")
                            && !open_content.has_attribute("inert")
                            && task_host
                                .query_selector_all("[data-slot='accordion-content']")
                                .expect("content query should succeed")
                                .length()
                                == 1,
                    );

                    set_open.set(false).expect("accordion should close");
                    flush_browser_tasks().await;
                    let closed_content = content(&task_host);
                    valid_for_task.set(
                        valid_for_task.get()
                            && initial_content_for_task.is_same_node(Some(closed_content.as_ref()))
                            && initial_input_for_task
                                .is_same_node(Some(child_input(&closed_content).as_ref()))
                            && closed_content.get_attribute("data-state").as_deref()
                                == Some("closed")
                            && closed_content.get_attribute("aria-hidden").as_deref()
                                == Some("true")
                            && closed_content.has_attribute("inert"),
                    );

                    set_open.set(true).expect("accordion should reopen");
                    flush_browser_tasks().await;
                    let reopened_content = content(&task_host);
                    let reopened_input = child_input(&reopened_content);
                    valid_for_task.set(
                        valid_for_task.get()
                            && initial_content_for_task
                                .is_same_node(Some(reopened_content.as_ref()))
                            && initial_input_for_task.is_same_node(Some(reopened_input.as_ref()))
                            && reopened_input.value() == "typed",
                    );
                    completed_for_task.set(true);
                },
                error_handler.view(),
            )
            .expect("accordion toggle task should register");
        (completed, valid, errors)
    });

    for _ in 0..12 {
        flush_browser_tasks().await;
        if completed.get() {
            break;
        }
    }
    assert!(completed.get(), "accordion toggle task should complete");
    assert!(
        valid.get(),
        "accordion wrapper and child identity should be stable"
    );
    assert_eq!(
        errors.get(),
        0,
        "accordion should not report runtime errors"
    );

    root.close()
        .expect("accordion owner cleanup should succeed");
    assert!(
        host.first_child().is_none(),
        "accordion host should be empty after cleanup"
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test(async)]
async fn unmount_mode_keeps_wrapper_but_recreates_content_slot() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");
    let task_host = host.clone();

    let (completed, valid, errors) = root.with_access(|owner| {
        let (open, set_open) = owner
            .signal(false)
            .expect("accordion open signal should be created");
        let errors = Rc::new(Cell::new(0usize));
        let errors_for_handler = errors.clone();
        let error_handler = owner
            .error_handler(move |_| {
                errors_for_handler.set(errors_for_handler.get().saturating_add(1));
            })
            .expect("test error handler should be registered");
        let clicks = Rc::new(Cell::new(0usize));
        let clicks_for_handler = clicks.clone();
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = AccordionContent(
            ctx,
            chain!(
                input()
                    .attr("data-test", "accordion-input")
                    .attr("value", "initial"),
                button("Action")
                    .attr("data-test", "accordion-slot-button")
                    .on_click(move |_| {
                        clicks_for_handler.set(clicks_for_handler.get().saturating_add(1));
                        Ok(())
                    })
            ),
        )
        .open(open)
        .expect("content open prop should be created")
        .mode(AccordionContentMode::UnmountWhenClosed)
        .build()
        .expect("content should build");
        let mount_owner = MountOwnerToken::new(owner);
        let _ = view
            .mount(
                &mount_owner,
                host.as_ref(),
                Vec::new(),
                error_handler.view(),
            )
            .expect("unmount mode content should mount");

        let initial_content = content(&host);
        assert!(
            initial_content
                .query_selector("[data-test='accordion-input']")
                .expect("input selector should be valid")
                .is_none()
        );
        let completed = Rc::new(Cell::new(false));
        let valid = Rc::new(Cell::new(true));
        let completed_for_task = completed.clone();
        let valid_for_task = valid.clone();
        let clicks_for_task = clicks.clone();
        let initial_content_for_task = initial_content.clone();
        let task_host = task_host.clone();
        owner
            .spawn_scoped(
                async move {
                    set_open.set(true).expect("accordion should open");
                    flush_browser_tasks().await;
                    let open_content = content(&task_host);
                    let open_input = child_input(&open_content);
                    let old_button = slot_button(&open_content);
                    open_input.set_value("typed");
                    dispatch_click(&old_button);
                    flush_browser_tasks().await;
                    valid_for_task.set(
                        valid_for_task.get()
                            && initial_content_for_task.is_same_node(Some(open_content.as_ref()))
                            && clicks_for_task.get() == 1,
                    );

                    set_open.set(false).expect("accordion should close");
                    flush_browser_tasks().await;
                    let closed_content = content(&task_host);
                    valid_for_task.set(
                        valid_for_task.get()
                            && initial_content_for_task.is_same_node(Some(closed_content.as_ref()))
                            && closed_content
                                .query_selector("[data-test='accordion-input']")
                                .expect("input selector should be valid")
                                .is_none()
                            && closed_content
                                .query_selector("[data-test='accordion-slot-button']")
                                .expect("slot button selector should be valid")
                                .is_none(),
                    );
                    dispatch_click(&old_button);
                    flush_browser_tasks().await;
                    valid_for_task.set(valid_for_task.get() && clicks_for_task.get() == 1);

                    set_open.set(true).expect("accordion should reopen");
                    flush_browser_tasks().await;
                    let reopened_content = content(&task_host);
                    let reopened_input = child_input(&reopened_content);
                    let reopened_button = slot_button(&reopened_content);
                    dispatch_click(&reopened_button);
                    flush_browser_tasks().await;
                    valid_for_task.set(
                        valid_for_task.get()
                            && initial_content_for_task
                                .is_same_node(Some(reopened_content.as_ref()))
                            && !open_input.is_same_node(Some(reopened_input.as_ref()))
                            && !old_button.is_same_node(Some(reopened_button.as_ref()))
                            && reopened_input.value() == "initial"
                            && clicks_for_task.get() == 2,
                    );

                    set_open.set(false).expect("accordion should close again");
                    flush_browser_tasks().await;
                    completed_for_task.set(true);
                },
                error_handler.view(),
            )
            .expect("unmount mode task should register");
        (completed, valid, errors)
    });

    for _ in 0..12 {
        flush_browser_tasks().await;
        if completed.get() {
            break;
        }
    }
    assert!(completed.get(), "unmount mode task should complete");
    assert!(valid.get(), "unmount mode slot should reset independently");
    assert_eq!(
        errors.get(),
        0,
        "accordion should not report runtime errors"
    );

    root.close()
        .expect("accordion owner cleanup should succeed");
    assert!(
        host.first_child().is_none(),
        "accordion host should be empty after cleanup"
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

struct FailingAccordionView;

impl<'scope> View<'scope> for FailingAccordionView {
    fn mount(
        &self,
        _owner: &dyn MountOwner<'scope>,
        _parent: &web_sys::Node,
        _attrs: Vec<AttrOp<'scope>>,
        _error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        Err(SilexError::fatal(SilexErrorKind::Dom(
            "intentional Accordion slot mount failure".to_string(),
        )))
    }
}

#[wasm_bindgen_test]
fn unmount_mode_mount_failure_rolls_back_wrapper_and_slot() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (open, _) = owner
            .signal(true)
            .expect("accordion open signal should be created");
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = AccordionContent(ctx, FailingAccordionView)
            .open(open)
            .expect("content open prop should be created")
            .mode(AccordionContentMode::UnmountWhenClosed)
            .build()
            .expect("content should build");
        let mount_owner = MountOwnerToken::new(owner);
        assert!(
            view.mount(
                &mount_owner,
                host.as_ref(),
                Vec::new(),
                error_handler.view(),
            )
            .is_err()
        );
        assert!(host.first_child().is_none());
    });

    root.close()
        .expect("accordion owner cleanup should succeed");
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}
