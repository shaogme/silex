#![cfg(target_arch = "wasm32")]

use js_sys::Promise;
use silex::components::{Portal, PortalContentMode};
use silex::prelude::*;
use silex::ui::{
    Dialog, Popover, PopoverContent, PopoverTrigger, Tooltip, TooltipContent, TooltipTrigger,
};
use silex_dom::document;
use silex_dom::view::{MountContext, MountInstance, MountOwnerToken, View};
use std::rc::Rc;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
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

async fn wait_ms(milliseconds: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        web_sys::window()
            .expect("window should exist")
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref(),
                milliseconds,
            )
            .expect("timeout should be scheduled");
    });
    JsFuture::from(promise)
        .await
        .expect("timeout promise should resolve");
}

fn dispatch_mouse_event(element: &Element, name: &str) {
    let event = web_sys::MouseEvent::new(name).expect("mouse event should be creatable");
    element
        .dispatch_event(&event)
        .expect("mouse event should dispatch");
}

fn select_text(element: &Element) -> String {
    let range = document()
        .create_range()
        .expect("selection range should be creatable");
    range
        .select_node_contents(element)
        .expect("selection range should contain tooltip content");
    let selection = web_sys::window()
        .expect("window should exist")
        .get_selection()
        .expect("selection should be readable")
        .expect("window should expose a selection");
    selection
        .remove_all_ranges()
        .expect("selection should clear");
    selection
        .add_range(&range)
        .expect("selection range should be applied");
    selection.to_string().into()
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

fn mount_view<'scope, V: View<'scope>>(
    view: &V,
    owner: &MountOwnerToken<'scope>,
    parent: &Element,
    error_handler: silex_core::ErrorReporter<'scope>,
) -> SilexResult<MountInstance<'scope>> {
    let context = MountContext::for_parent(parent.clone().into(), owner.clone(), error_handler);
    let instance = view.mount(&context, Vec::new())?;
    context.transaction().commit()?;
    Ok(instance)
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

fn portal_host() -> Element {
    document()
        .query_selector("body > div[data-portal-host]")
        .expect("portal host selector should be valid")
        .expect("portal host should be mounted")
        .dyn_into::<Element>()
        .expect("portal host should be an element")
}

fn portal_host_count() -> u32 {
    document()
        .query_selector_all("body > div[data-portal-host]")
        .expect("portal host selector should be valid")
        .length()
}

fn portal_content() -> Element {
    document()
        .query_selector("body > div[data-portal-host] [data-test=portal-content]")
        .expect("portal content selector should be valid")
        .expect("portal content should be mounted")
        .dyn_into::<Element>()
        .expect("portal content should be an element")
}

struct FailingView;

impl<'scope> View<'scope> for FailingView {
    fn mount(
        &self,
        _context: &MountContext<'scope>,
        _attrs: Vec<AttrOp<'scope>>,
    ) -> SilexResult<MountInstance<'scope>> {
        Err(SilexError::fatal(SilexErrorKind::Dom(
            "intentional Portal mount failure".to_string(),
        )))
    }
}

struct PanickingView;

impl<'scope> View<'scope> for PanickingView {
    fn mount(
        &self,
        _context: &MountContext<'scope>,
        _attrs: Vec<AttrOp<'scope>>,
    ) -> SilexResult<MountInstance<'scope>> {
        panic!("intentional Portal mount panic")
    }
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
            Portal(ctx, show_modal)
                .children(
                    div![
                        h4("I am a Modal!"),
                        p("I am rendered via Portal directly into the body, but I share ctx!")
                    ]
                    .attr("data-test", "portal-content"),
                )
                .attr("data-portal-host", "")
                .build(),
        ];
        let mount_owner = MountOwnerToken::new(owner);
        let _ = mount_view(&view, &mount_owner, &host, error_handler.view())
            .expect("portal demo should mount");
        let completed_for_task = completed.clone();
        let valid_for_task = valid.clone();
        let initial_host = portal_host();
        valid.set(
            valid.get()
                && portal_host_count() == 1
                && initial_host.get_attribute("data-state").as_deref() == Some("closed")
                && initial_host.has_attribute("hidden"),
        );
        owner
            .spawn_scoped(
                async move {
                    for _ in 0..3 {
                        set_show_modal.set(true).expect("modal should open");
                        flush_browser_tasks().await;
                        let current_host = portal_host();
                        valid_for_task.set(
                            valid_for_task.get()
                                && initial_host.is_same_node(Some(current_host.as_ref()))
                                && portal_host_count() == 1
                                && current_host.get_attribute("data-state").as_deref()
                                    == Some("open")
                                && !current_host.has_attribute("hidden")
                                && text_occurrence_count("body h4", "I am a Modal!") == 1
                                && text_occurrence_count(
                                    "body p",
                                    "I am rendered via Portal directly into the body, but I share ctx!",
                                ) == 1,
                        );
                        set_show_modal.set(false).expect("modal should close");
                        flush_browser_tasks().await;
                        let current_host = portal_host();
                        valid_for_task.set(
                            valid_for_task.get()
                                && initial_host.is_same_node(Some(current_host.as_ref()))
                                && portal_host_count() == 1
                                && current_host.get_attribute("data-state").as_deref()
                                    == Some("closed")
                                && current_host.has_attribute("hidden")
                                && text_occurrence_count("body h4", "I am a Modal!") == 1
                                && text_occurrence_count(
                                    "body p",
                                    "I am rendered via Portal directly into the body, but I share ctx!",
                                ) == 1,
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
        "portal host and content identity should remain stable across toggles"
    );
    assert_eq!(portal_host_count(), 1);
    assert_eq!(text_occurrence_count("body h4", "I am a Modal!"), 1);
    assert_eq!(
        text_occurrence_count(
            "body p",
            "I am rendered via Portal directly into the body, but I share ctx!"
        ),
        1
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
    assert_eq!(portal_host_count(), 0);
}

#[wasm_bindgen_test(async)]
async fn tooltip_mouse_crossing_keeps_host_wrapper_and_content_identity() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = Tooltip(ctx, move |context| {
            let trigger = TooltipTrigger(ctx, button("Trigger"))
                .context(context)
                .build()?;
            let content = TooltipContent(ctx, p("Selectable tooltip text"))
                .context(context)
                .build()?;
            Ok(chain!(trigger, content).into_any())
        })
        .build()
        .expect("tooltip should build");
        let mount_owner = MountOwnerToken::new(owner);
        let _ = mount_view(&view, &mount_owner, &host, error_handler.view())
            .expect("tooltip should mount");
    });

    let trigger = document()
        .query_selector("[data-slot=tooltip-trigger]")
        .expect("tooltip trigger selector should be valid")
        .expect("tooltip trigger should be mounted")
        .dyn_into::<Element>()
        .expect("tooltip trigger should be an element");
    let portal = document()
        .query_selector("body > div[data-portal-host=tooltip]")
        .expect("tooltip Portal selector should be valid")
        .expect("tooltip Portal should be mounted before opening")
        .dyn_into::<Element>()
        .expect("tooltip Portal should be an element");
    let wrapper = portal
        .query_selector("[data-radix-popper-content-wrapper]")
        .expect("tooltip wrapper selector should be valid")
        .expect("tooltip wrapper should be mounted")
        .dyn_into::<Element>()
        .expect("tooltip wrapper should be an element");
    let content = portal
        .query_selector("[data-slot=tooltip-content]")
        .expect("tooltip content selector should be valid")
        .expect("tooltip content should be mounted")
        .dyn_into::<Element>()
        .expect("tooltip content should be an element");
    assert!(portal.has_attribute("hidden"));

    dispatch_mouse_event(&trigger, "mouseenter");
    flush_browser_tasks().await;
    let open_portal = document()
        .query_selector("body > div[data-portal-host=tooltip]")
        .expect("tooltip Portal selector should be valid")
        .expect("tooltip Portal should remain mounted")
        .dyn_into::<Element>()
        .expect("tooltip Portal should be an element");
    assert!(portal.is_same_node(Some(open_portal.as_ref())));
    assert_eq!(portal.get_attribute("data-state").as_deref(), Some("open"));
    assert!(!portal.has_attribute("hidden"));

    dispatch_mouse_event(&trigger, "mouseleave");
    dispatch_mouse_event(&content, "mouseenter");
    flush_browser_tasks().await;
    assert!(
        portal.is_same_node(Some(
            &document()
                .query_selector("body > div[data-portal-host=tooltip]")
                .expect("tooltip Portal selector should be valid")
                .expect("tooltip Portal should remain mounted")
        ))
    );
    assert!(
        wrapper.is_same_node(Some(
            &portal
                .query_selector("[data-radix-popper-content-wrapper]")
                .expect("tooltip wrapper selector should be valid")
                .expect("tooltip wrapper should remain mounted")
        ))
    );
    assert!(
        content.is_same_node(Some(
            &portal
                .query_selector("[data-slot=tooltip-content]")
                .expect("tooltip content selector should be valid")
                .expect("tooltip content should remain mounted")
        ))
    );
    assert_eq!(select_text(&content).trim(), "Selectable tooltip text");

    dispatch_mouse_event(&content, "mouseleave");
    wait_ms(220).await;
    assert!(
        portal.is_same_node(Some(
            &document()
                .query_selector("body > div[data-portal-host=tooltip]")
                .expect("tooltip Portal selector should be valid")
                .expect("tooltip Portal should remain mounted")
        ))
    );
    assert_eq!(
        portal.get_attribute("data-state").as_deref(),
        Some("closed")
    );
    assert!(portal.has_attribute("hidden"));

    root.close().expect("root cleanup should succeed");
    assert_eq!(
        document()
            .query_selector("body > div[data-portal-host=tooltip]")
            .expect("tooltip Portal selector should be valid")
            .is_none(),
        true
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test(async)]
async fn popover_keeps_host_and_overlay_stable_across_click_outside() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = Popover(ctx, move |context| {
            let trigger = PopoverTrigger(ctx, button("Open popover"))
                .context(context)
                .build()?;
            let content = PopoverContent(ctx, p("Popover content"))
                .context(context)
                .build()?;
            Ok(chain!(trigger, content).into_any())
        })
        .build()
        .expect("popover should build");
        let mount_owner = MountOwnerToken::new(owner);
        let _ = mount_view(&view, &mount_owner, &host, error_handler.view())
            .expect("popover should mount");
    });

    let trigger = document()
        .query_selector("[data-slot=popover-trigger]")
        .expect("popover trigger selector should be valid")
        .expect("popover trigger should be mounted")
        .dyn_into::<Element>()
        .expect("popover trigger should be an element");
    let portal = document()
        .query_selector("body > div[data-portal-host=popover]")
        .expect("popover Portal selector should be valid")
        .expect("popover Portal should be mounted before opening")
        .dyn_into::<Element>()
        .expect("popover Portal should be an element");
    let wrapper = portal
        .query_selector("[data-radix-popper-content-wrapper]")
        .expect("popover wrapper selector should be valid")
        .expect("popover wrapper should be mounted")
        .dyn_into::<Element>()
        .expect("popover wrapper should be an element");
    let overlay = portal
        .query_selector("[data-slot=popover-overlay]")
        .expect("popover overlay selector should be valid")
        .expect("popover overlay should be mounted")
        .dyn_into::<Element>()
        .expect("popover overlay should be an element");
    assert!(portal.has_attribute("hidden"));

    dispatch_mouse_event(&trigger, "click");
    flush_browser_tasks().await;
    let open_portal = document()
        .query_selector("body > div[data-portal-host=popover]")
        .expect("popover Portal selector should be valid")
        .expect("popover Portal should remain mounted")
        .dyn_into::<Element>()
        .expect("popover Portal should be an element");
    assert!(portal.is_same_node(Some(open_portal.as_ref())));
    assert_eq!(portal.get_attribute("data-state").as_deref(), Some("open"));
    assert!(!portal.has_attribute("hidden"));
    let content = portal
        .query_selector("[data-slot=popover-content]")
        .expect("popover content selector should be valid")
        .expect("popover content should be mounted")
        .dyn_into::<Element>()
        .expect("popover content should be an element");
    assert_eq!(content.get_attribute("data-state").as_deref(), Some("open"));

    dispatch_mouse_event(&overlay, "click");
    flush_browser_tasks().await;
    assert!(
        portal.is_same_node(Some(
            &document()
                .query_selector("body > div[data-portal-host=popover]")
                .expect("popover Portal selector should be valid")
                .expect("popover Portal should remain mounted")
        ))
    );
    assert_eq!(
        portal.get_attribute("data-state").as_deref(),
        Some("closed")
    );
    assert!(portal.has_attribute("hidden"));
    assert!(
        overlay.is_same_node(Some(
            &portal
                .query_selector("[data-slot=popover-overlay]")
                .expect("popover overlay selector should be valid")
                .expect("popover overlay should remain mounted")
        ))
    );
    assert!(
        wrapper.is_same_node(Some(
            &portal
                .query_selector("[data-radix-popper-content-wrapper]")
                .expect("popover wrapper selector should be valid")
                .expect("popover wrapper should remain mounted")
        ))
    );

    dispatch_mouse_event(&trigger, "click");
    flush_browser_tasks().await;
    assert!(!portal.has_attribute("hidden"));
    assert!(
        content.is_same_node(Some(
            &portal
                .query_selector("[data-slot=popover-content]")
                .expect("popover content selector should be valid")
                .expect("popover content should remain mounted")
        ))
    );

    root.close().expect("root cleanup should succeed");
    assert!(
        document()
            .query_selector("body > div[data-portal-host=popover]")
            .expect("popover Portal selector should be valid")
            .is_none()
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test(async)]
async fn dialog_restores_focus_and_keeps_host_stable_across_overlay_close() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (open, set_open) = owner
            .signal(false)
            .expect("dialog signal should be created");
        let on_close = owner
            .callback(move |_| {
                set_open.set(false)?;
                Ok(())
            })
            .expect("dialog close callback should be created");
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = div(chain!(
            button("Open dialog")
                .attr("data-slot", "dialog-trigger")
                .on_click(move |_| {
                    set_open.set(true)?;
                    Ok(())
                }),
            Dialog(ctx, chain!(p("Dialog content")))
                .open(open)
                .expect("dialog open property should apply")
                .on_close(on_close)
                .build()
                .expect("dialog should build")
        ));
        let mount_owner = MountOwnerToken::new(owner);
        let _ = mount_view(&view, &mount_owner, &host, error_handler.view())
            .expect("dialog should mount");
    });

    let trigger = document()
        .query_selector("[data-slot=dialog-trigger]")
        .expect("dialog trigger selector should be valid")
        .expect("dialog trigger should be mounted")
        .dyn_into::<Element>()
        .expect("dialog trigger should be an element");
    let trigger_html = trigger
        .dyn_ref::<web_sys::HtmlElement>()
        .expect("dialog trigger should be an HTML element");
    trigger_html
        .focus()
        .expect("dialog trigger should receive focus");

    let portal = document()
        .query_selector("body > div[data-portal-host=dialog]")
        .expect("dialog Portal selector should be valid")
        .expect("dialog Portal should be mounted before opening")
        .dyn_into::<Element>()
        .expect("dialog Portal should be an element");
    let content = portal
        .query_selector("[data-slot=dialog-content]")
        .expect("dialog content selector should be valid")
        .expect("dialog content should be mounted")
        .dyn_into::<Element>()
        .expect("dialog content should be an element");
    let overlay = portal
        .query_selector("[data-slot=dialog-overlay]")
        .expect("dialog overlay selector should be valid")
        .expect("dialog overlay should be mounted")
        .dyn_into::<Element>()
        .expect("dialog overlay should be an element");
    assert!(portal.has_attribute("hidden"));
    assert_eq!(
        portal.get_attribute("data-state").as_deref(),
        Some("closed")
    );
    assert_eq!(
        portal
            .dyn_ref::<web_sys::HtmlElement>()
            .expect("dialog Portal should be an HTML element")
            .style()
            .get_property_value("pointer-events")
            .expect("dialog pointer-events should be readable"),
        "none"
    );

    dispatch_mouse_event(&trigger, "click");
    flush_browser_tasks().await;
    let active = document()
        .active_element()
        .expect("dialog should focus its content when opened");
    assert!(content.is_same_node(Some(active.as_ref())));
    assert_eq!(portal.get_attribute("data-state").as_deref(), Some("open"));
    assert!(!portal.has_attribute("hidden"));
    assert_eq!(
        portal
            .dyn_ref::<web_sys::HtmlElement>()
            .expect("dialog Portal should be an HTML element")
            .style()
            .get_property_value("pointer-events")
            .expect("dialog pointer-events should be readable"),
        "auto"
    );

    dispatch_mouse_event(&overlay, "click");
    flush_browser_tasks().await;
    let closed_portal = document()
        .query_selector("body > div[data-portal-host=dialog]")
        .expect("dialog Portal selector should be valid")
        .expect("dialog Portal should remain mounted after close")
        .dyn_into::<Element>()
        .expect("dialog Portal should be an element");
    let active = document()
        .active_element()
        .expect("dialog should restore trigger focus when closed");
    assert!(portal.is_same_node(Some(closed_portal.as_ref())));
    assert!(trigger.is_same_node(Some(active.as_ref())));
    assert!(portal.has_attribute("hidden"));

    dispatch_mouse_event(&trigger, "click");
    flush_browser_tasks().await;
    let reopened_content = portal
        .query_selector("[data-slot=dialog-content]")
        .expect("dialog content selector should be valid")
        .expect("dialog content should remain mounted")
        .dyn_into::<Element>()
        .expect("dialog content should be an element");
    assert!(content.is_same_node(Some(reopened_content.as_ref())));
    assert!(
        content.is_same_node(Some(
            &document()
                .active_element()
                .expect("dialog should focus content again")
        ))
    );

    root.close().expect("root cleanup should succeed");
    assert!(
        document()
            .query_selector("body > div[data-portal-host=dialog]")
            .expect("dialog Portal selector should be valid")
            .is_none()
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test(async)]
async fn portal_unmount_mode_keeps_host_and_unmounts_only_content() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    let (completed, valid, errors) = root.with_access(|owner| {
        let (open, set_open) = owner.signal(false).expect("open signal should be created");
        let errors = Rc::new(Cell::new(0));
        let completed = Rc::new(Cell::new(false));
        let valid = Rc::new(Cell::new(true));
        let errors_for_handler = errors.clone();
        let error_handler = owner
            .error_handler(move |_| errors_for_handler.set(errors_for_handler.get() + 1))
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = Portal(ctx, open)
            .children(div("Unmounted content").attr("data-test", "portal-content"))
            .content_mode(PortalContentMode::UnmountWhenClosed)
            .attr("data-portal-host", "")
            .build();
        let mount_owner = MountOwnerToken::new(owner);
        let _ = mount_view(&view, &mount_owner, &host, error_handler.view())
            .expect("unmount mode portal should mount");
        assert_eq!(portal_host_count(), 1);
        assert!(
            document()
                .query_selector("body > div[data-portal-host] [data-test=portal-content]")
                .expect("portal content selector should be valid")
                .is_none()
        );
        let completed_for_task = completed.clone();
        let valid_for_task = valid.clone();
        owner
            .spawn_scoped(
                async move {
                    set_open.set(true).expect("portal should open");
                    flush_browser_tasks().await;
                    let host_when_open = portal_host();
                    let content_when_open = portal_content();
                    valid_for_task.set(
                        valid_for_task.get()
                            && host_when_open.get_attribute("data-state").as_deref()
                                == Some("open"),
                    );

                    set_open.set(false).expect("portal should close");
                    flush_browser_tasks().await;
                    valid_for_task.set(
                        valid_for_task.get()
                            && document()
                                .query_selector(
                                    "body > div[data-portal-host] [data-test=portal-content]",
                                )
                                .expect("portal content selector should be valid")
                                .is_none()
                            && host_when_open.is_same_node(Some(portal_host().as_ref())),
                    );

                    set_open.set(true).expect("portal should reopen");
                    flush_browser_tasks().await;
                    valid_for_task.set(
                        valid_for_task.get()
                            && host_when_open.is_same_node(Some(portal_host().as_ref()))
                            && !content_when_open.is_same_node(Some(portal_content().as_ref()))
                            && portal_host_count() == 1,
                    );
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
    assert!(
        valid.get(),
        "unmount mode lifecycle should remain consistent"
    );
    assert_eq!(errors.get(), 0);

    root.close().expect("root cleanup should succeed");
    assert_eq!(portal_host_count(), 0);
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test]
fn portal_mount_failure_leaves_no_host() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (open, _) = owner.signal(false).expect("open signal should be created");
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let failing = Portal(ctx, open)
            .children(FailingView)
            .attr("data-portal-host", "")
            .build();
        let mount_owner = MountOwnerToken::new(owner);
        assert!(mount_view(&failing, &mount_owner, &host, error_handler.view()).is_err());
        assert_eq!(portal_host_count(), 0);
    });

    root.close().expect("root cleanup should succeed");
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[ignore = "stable wasm aborts before catch_unwind; run with nightly build-std"]
#[wasm_bindgen_test]
fn portal_mount_panic_leaves_no_host() {
    let host = host();
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let (open, _) = owner.signal(false).expect("open signal should be created");
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let panicking = Portal(ctx, open)
            .children(PanickingView)
            .attr("data-portal-host", "")
            .build();
        let mount_owner = MountOwnerToken::new(owner);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = mount_view(&panicking, &mount_owner, &host, error_handler.view());
        }));
        assert!(result.is_err(), "Portal should rethrow child mount panics");
        assert_eq!(portal_host_count(), 0);
    });

    root.close().expect("root cleanup should succeed");
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}

#[wasm_bindgen_test]
fn portal_host_respects_explicit_target_and_cleanup() {
    let host = host();
    let target = document()
        .create_element("section")
        .expect("target should be creatable");
    host.append_child(&target)
        .expect("target should be attached to test host");
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root runtime should start");

    root.with_access(|owner| {
        let error_handler = owner
            .error_handler(|_| {})
            .expect("test error handler should be registered");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = PortalHost(ctx)
            .children(div("target content"))
            .mount_to(Some(target.clone().into()))
            .attr("data-portal-host", "")
            .build();
        let mount_owner = MountOwnerToken::new(owner);
        let _ = mount_view(&view, &mount_owner, &host, error_handler.view())
            .expect("target PortalHost should mount");
        assert_eq!(portal_host_count(), 0);
        assert_eq!(
            target
                .query_selector("[data-portal-host]")
                .expect("target host selector should be valid")
                .map(|_| 1),
            Some(1)
        );
    });

    root.close().expect("root cleanup should succeed");
    assert!(
        target
            .query_selector("[data-portal-host]")
            .expect("target host selector should be valid")
            .is_none()
    );
    host.parent_node()
        .expect("test host should be attached")
        .remove_child(&host)
        .expect("test host should be detached");
}
