use silex_core::reactivity::Signal;
use silex_core::{ReactiveError, Runtime, RxGet, SilexError, SilexErrorKind};
use silex_dom::{adapters::ssr::SsrDom, lifecycle::CleanupSink};
use silex_view::app::MountedApp;
use silex_view::attributes::{AttributeBuilder, GlobalAttributes, GlobalEventAttributes};
use silex_view::elements::AnyView;
use silex_view::elements::{Element, Tag, TagMetadata, TagNamespace, TypedElement};
use silex_view::flow::{BranchEvaluation, DynamicRenderer};
use silex_view::flow::{IndexedListView, RenderOnlyKeyedListView, StableBranch};
use silex_view::mount::{MountContext, MountInstance, View};
use std::cell::Cell;
use std::rc::Rc;

fn app(dom: &SsrDom) -> MountedApp {
    let host = dom.document().expect("SSR document").node().clone();
    MountedApp::new(
        Runtime::new(),
        dom.context(),
        host,
        CleanupSink::new(|_| {}),
    )
}

#[derive(Clone, Copy)]
struct SvgRect;

impl Tag for SvgRect {
    const METADATA: TagMetadata = TagMetadata::new("rect", TagNamespace::Svg, false);
}

#[derive(Clone, Copy)]
struct HtmlInput;

impl Tag for HtmlInput {
    const METADATA: TagMetadata = TagMetadata::new("input", TagNamespace::Html, true);
}

struct FailingChild;

impl<'scope> View<'scope> for FailingChild {
    fn mount(
        &self,
        context: &MountContext<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        let _ = context.mount(&"provisional child")?;
        Err(SilexError::fatal(SilexErrorKind::Framework(
            "intentional child failure".into(),
        )))
    }
}

struct BorrowingDispatchView;

impl<'scope> View<'scope> for BorrowingDispatchView {
    fn mount(
        &self,
        context: &MountContext<'scope>,
    ) -> silex_core::SilexResult<MountInstance<'scope>> {
        let instance = context.mount(&"borrowed")?;
        context.mount_unit(&"unit")?;
        Ok(instance)
    }
}

#[test]
fn mount_context_dispatches_borrowed_views_and_units() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            context.mount_unit(BorrowingDispatchView, handler.view())
        })
        .expect("borrowed dispatch should mount");

    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "borrowedunit"
    );
}

#[test]
fn primitive_any_view_and_typed_elements_mount_with_metadata() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let children = vec![
                AnyView::from("text"),
                AnyView::from(42_i32),
                AnyView::from(Some("optional")),
                AnyView::from(None::<&str>),
            ];
            let view = Element::with_child("main", children);
            context.mount_unit(view, handler.view())
        })
        .expect("primitive views should mount");

    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<main>text42optional</main>"
    );
    mounted.dispose().expect("dispose should succeed");

    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let view = TypedElement::<SvgRect>::with_child_from_tag("square");
            context.mount_unit(view, handler.view())
        })
        .expect("typed SVG element should mount");
    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<rect xmlns=\"http://www.w3.org/2000/svg\">square</rect>"
    );
    mounted.dispose().expect("dispose should remove SVG view");

    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let view = TypedElement::<HtmlInput>::from_tag();
            context.mount_unit(view, handler.view())
        })
        .expect("typed void element should mount");
    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<input>"
    );
}

#[test]
fn composite_mount_returns_all_nodes_and_disposes_them() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let node_count = Rc::new(Cell::new(0));
    let node_count_for_assertion = node_count.clone();
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let instance = context.mount(
                vec![
                    Element::with_child("span", "first"),
                    Element::with_child("span", "second"),
                ],
                handler.view(),
            )?;
            node_count_for_assertion.set(instance.len());
            Ok(())
        })
        .expect("composite view should mount");

    assert_eq!(node_count.get(), 2);
    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<span>first</span><span>second</span>"
    );
    mounted
        .dispose()
        .expect("dispose should remove composite nodes");
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn element_child_failure_rolls_back_provisional_nodes_and_owner() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let failure = mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let node_ref = context.owner().node_ref();
            let result = context.mount_unit(
                Element::with_child("section", FailingChild).node_ref(node_ref.clone()),
                handler.view(),
            );
            assert!(
                node_ref
                    .get()
                    .expect("node ref should be readable")
                    .is_none(),
                "provisional rollback must clear the binding"
            );
            result
        })
        .expect_err("failing child should fail the element mount");

    assert!(matches!(
        failure.kind(),
        SilexErrorKind::View(view)
            if view.mount_error().is_some_and(|error| error.can_retry())
    ));
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            context.mount_unit(Element::with_child("p", "recovered"), handler.view())
        })
        .expect("mount should remain retryable");
    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<p>recovered</p>"
    );
}

#[test]
fn dynamic_node_ref_replacement_keeps_the_new_binding() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let observed = Rc::new(Cell::new(0_u64));
    let observed_for_assertion = observed.clone();
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let show_second = context.access().signal(false).expect("branch signal");
            let node_ref = context.owner().node_ref();
            let node_ref_for_renderer = node_ref.clone();
            let renderer = DynamicRenderer::new(move |render_context| {
                let tag = if show_second.get()? {
                    "second"
                } else {
                    "first"
                };
                render_context.mount(&Element::new(tag).node_ref(node_ref_for_renderer.clone()))
            });
            context.mount_unit(renderer, handler.view())?;
            observed_for_assertion.set(
                node_ref
                    .get()
                    .expect("first binding should be readable")
                    .expect("first binding should be present")
                    .identity(),
            );
            show_second.set(true)?;
            let current = node_ref
                .get()
                .expect("replacement binding should be readable")
                .expect("replacement binding should be present")
                .identity();
            assert_ne!(current, observed_for_assertion.get());
            Ok(())
        })
        .expect("dynamic ref replacement should mount");
    assert!(observed.get() > 0);
    mounted.dispose().expect("dispose should succeed");
}

#[test]
fn mount_dom_action_resolves_node_ref_and_is_closed_with_owner() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let action_closed = Rc::new(Cell::new(false));
    let action_closed_for_assertion = action_closed.clone();
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let node_ref = context.owner().node_ref();
            let action = context.dom_action();
            context.mount_unit(
                Element::new("button").node_ref(node_ref.clone()),
                handler.view(),
            )?;
            let resolved = action
                .with_context(|dom| node_ref.resolve_element(dom))?
                .expect("mount action should resolve the bound element");
            assert_eq!(
                resolved.node().identity(),
                node_ref
                    .get()
                    .expect("binding should be readable")
                    .expect("binding")
                    .identity()
            );
            let focus_error = action
                .with_context(|dom| node_ref.focus(dom))
                .expect_err("SSR focus must report unsupported");
            assert!(format!("{focus_error}").contains("unsupported capability: focus"));
            let action_closed_for_cleanup = action_closed.clone();
            context.owner().on_cleanup(
                Box::new(move || {
                    let error = action
                        .with_context(|dom| node_ref.resolve_element(dom))
                        .expect_err("closed owner must gate the mount action");
                    action_closed_for_cleanup.set(matches!(
                        error.kind(),
                        SilexErrorKind::Reactivity(ReactiveError::NoSuchNode)
                    ));
                    Ok(())
                }),
                handler.view(),
            )
        })
        .expect("mount action should mount");
    mounted.dispose().expect("dispose should succeed");
    assert!(action_closed_for_assertion.get());
}

#[test]
fn mounted_app_rejects_cross_context_hosts_before_mounting() {
    let dom = SsrDom::new();
    let foreign = SsrDom::new();
    let foreign_host = foreign.document().expect("foreign document").node().clone();
    let result = MountedApp::try_new(
        Runtime::new(),
        dom.context(),
        foreign_host.clone(),
        CleanupSink::new(|_| {}),
    );
    assert!(result.is_err(), "try_new must reject a foreign host");

    let mut mounted = MountedApp::new(
        Runtime::new(),
        dom.context(),
        foreign_host,
        CleanupSink::new(|_| {}),
    );
    let error = mounted
        .mount(|_| Ok(()))
        .expect_err("new must reject the foreign host on the mount path");
    assert!(format!("{error}").contains("different contexts"));
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn attributes_are_consolidated_and_properties_are_not_serialized() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let active = context.access().signal(true).expect("active signal");
            let view = Element::new("input")
                .class("zeta")
                .class("alpha beta")
                .class_toggle("active", active)
                .style("color:red")
                .attr("data-value", "<&")
                .prop("value", "not an attribute")
                .attr("hidden", false);
            context.mount_unit(view, handler.view())
        })
        .expect("attribute view should mount");

    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<input class=\"active alpha beta zeta\" data-value=\"&lt;&amp;\" style=\"color: red;\">"
    );
}

#[test]
fn dynamic_view_and_stable_branch_follow_signal_changes() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let dynamic_value = context
                .access()
                .signal(String::from("before"))
                .expect("dynamic signal");
            let branch_key = context.access().signal(1_usize).expect("branch signal");
            let dynamic = move || {
                Element::with_child(
                    "span",
                    dynamic_value
                        .get()
                        .expect("dynamic value should be available"),
                )
            };
            context.mount_unit(dynamic, handler.view())?;
            context.mount_unit(
                StableBranch::new(
                    move || {
                        branch_key
                            .get()
                            .map(|value| BranchEvaluation::new(value, value))
                    },
                    |evaluation, _branch_context| {
                        let (key, value) = evaluation.into_parts();
                        AnyView::from(Element::with_child("output", format!("{key}:{value}")))
                    },
                ),
                handler.view(),
            )?;
            dynamic_value.set(String::from("after"))?;
            branch_key.set(2)
        })
        .expect("dynamic views should mount");

    let html = dom.serialize(Default::default()).expect("serialize");
    assert!(
        html.contains("<span>after</span>"),
        "unexpected HTML: {html}"
    );
    assert!(
        html.contains("<output>2:2</output>"),
        "unexpected HTML: {html}"
    );
    assert!(!html.contains("before"), "stale dynamic branch: {html}");
}

#[test]
fn stable_branch_preserves_state_when_key_evaluation_fails() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let errors = Rc::new(Cell::new(0_u32));
    let errors_for_assertion = errors.clone();

    mounted
        .mount({
            let errors = errors.clone();
            move |context| {
                let handler = context
                    .access()
                    .error_handler(move |_| errors.set(errors.get() + 1))
                    .expect("error handler");
                let fail = context.access().signal(false).expect("failure signal");
                let key = context.access().signal(1_usize).expect("key signal");
                context.mount_unit(
                    StableBranch::new(
                        move || {
                            if fail.get()? {
                                return Err(SilexError::fatal(SilexErrorKind::Framework(
                                    "intentional key failure".into(),
                                )));
                            }
                            key.get().map(|value| BranchEvaluation::new(value, value))
                        },
                        |evaluation, _| {
                            let (key, value) = evaluation.into_parts();
                            AnyView::from(Element::with_child("output", format!("{key}:{value}")))
                        },
                    ),
                    handler.view(),
                )?;
                fail.set(true)?;
                fail.set(false)?;
                key.set(2)
            }
        })
        .expect("stable branch should mount");

    assert!(errors_for_assertion.get() > 0);
    let html = dom.serialize(Default::default()).expect("serialize");
    assert!(
        html.contains("<output>2:2</output>"),
        "unexpected HTML: {html}"
    );
}

#[test]
fn dynamic_renderer_is_a_view_with_a_kernel_context_callback() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let value = context
                .access()
                .signal(String::from("before"))
                .expect("dynamic signal");
            let value_for_renderer = value;
            context.mount_unit(
                DynamicRenderer::new(move |context| {
                    let value = value_for_renderer.get()?;
                    let view = Element::with_child("output", value);
                    context.mount(&view)
                }),
                handler.view(),
            )?;
            value.set(String::from("after"))
        })
        .expect("dynamic renderer should mount");

    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<!--dyn-start--><output>after</output><!--dyn-end-->"
    );
}

#[test]
fn indexed_list_updates_length_and_disposes_with_the_app() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let values: Signal<'_, Vec<i32>> =
                context.access().signal(vec![1, 2]).expect("list signal");
            let list = IndexedListView::new(
                values,
                Rc::new(|value: i32, index| {
                    AnyView::from(Element::with_child("li", format!("{index}:{value}")))
                }),
            );
            context.mount_unit(list, handler.view())?;
            values.set(vec![3, 4, 5])
        })
        .expect("indexed list should mount");

    let html = dom.serialize(Default::default()).expect("serialize");
    assert!(html.contains("<li>0:3</li>"), "unexpected HTML: {html}");
    assert!(html.contains("<li>1:4</li>"), "unexpected HTML: {html}");
    assert!(html.contains("<li>2:5</li>"), "unexpected HTML: {html}");
    assert!(!html.contains("0:1"), "old list row survived: {html}");
    mounted.dispose().expect("dispose should succeed");
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
}

#[test]
fn render_only_keyed_list_updates_rows_when_reordered() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let values: Signal<'_, Vec<i32>> =
                context.access().signal(vec![1, 2, 3]).expect("list signal");
            let list = RenderOnlyKeyedListView::new(
                values,
                Rc::new(|value: &i32| *value),
                Rc::new(|value: i32, index| {
                    AnyView::from(Element::with_child("li", format!("{index}:{value}")))
                }),
                None,
            );
            context.mount_unit(list, handler.view())?;
            values.set(vec![3, 1, 2])
        })
        .expect("render-only keyed list should mount");

    let html = dom.serialize(Default::default()).expect("serialize");
    assert!(html.contains("<li>0:3</li>"), "unexpected list: {html}");
    assert!(html.contains("<li>1:1</li>"), "unexpected list: {html}");
    assert!(html.contains("<li>2:2</li>"), "unexpected list: {html}");
    assert!(!html.contains("0:1"), "stale keyed row survived: {html}");
}

#[test]
fn mount_can_retry_after_failure_and_remounts_cleanly() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let failure = mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            context.mount_unit(Element::with_child("div", "temporary"), handler.view())?;
            Err(SilexError::fatal(SilexErrorKind::Framework(
                "intentional failure".into(),
            )))
        })
        .expect_err("mount should fail");
    assert!(matches!(
        failure.kind(),
        SilexErrorKind::View(view)
            if view.mount_error().is_some_and(|error| error.can_retry())
    ));
    assert!(!mounted.is_poisoned());
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            context.mount_unit(Element::with_child("div", "recovered"), handler.view())
        })
        .expect("retry should succeed");
    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<div>recovered</div>"
    );

    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            context.mount_unit(Element::with_child("div", "replacement"), handler.view())
        })
        .expect("remount should dispose the previous session");
    assert_eq!(
        dom.serialize(Default::default()).expect("serialize"),
        "<div>replacement</div>"
    );
}

#[test]
fn panicking_mount_poisoning_keeps_host_empty() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let error = mounted
        .mount(|_| -> silex_core::SilexResult<()> { panic!("intentional panic") })
        .expect_err("panic should become a mount error");
    assert!(matches!(
        error.kind(),
        SilexErrorKind::View(view)
            if view.mount_error().is_some_and(|error| error.is_poisoned())
    ));
    assert!(mounted.is_poisoned());
    assert_eq!(dom.serialize(Default::default()).expect("serialize"), "");
    let retry_error = mounted
        .mount(|_| Ok(()))
        .expect_err("poisoned app should reject retry");
    assert!(matches!(
        retry_error.kind(),
        SilexErrorKind::View(view)
            if view.mount_error().is_some_and(|error| error.is_poisoned())
    ));
}

#[test]
fn node_ref_cleanup_and_reactive_attribute_cleanup_run_before_dom_removal() {
    let dom = SsrDom::new();
    let mut mounted = app(&dom);
    let cleared = Rc::new(Cell::new(false));
    let cleared_for_assertion = cleared.clone();
    let mut raw_node = None;
    mounted
        .mount(|context| {
            let handler = context.access().error_handler(|_| {}).expect("handler");
            let title = context
                .access()
                .signal(String::from("bound"))
                .expect("title signal");
            let node_ref = context.owner().node_ref();
            context.mount_unit(
                Element::with_child("div", "node")
                    .attr("title", title)
                    .node_ref(node_ref.clone()),
                handler.view(),
            )?;
            raw_node = node_ref.get().expect("node ref should be readable");
            let cleared_for_cleanup = cleared.clone();
            context.owner().on_cleanup(
                Box::new(move || {
                    cleared_for_cleanup.set(
                        node_ref
                            .get()
                            .expect("node ref should be readable")
                            .is_none(),
                    );
                    Ok(())
                }),
                handler.view(),
            )
        })
        .expect("view should mount");
    let raw_node = raw_node.expect("mounted node should be captured");
    mounted.dispose().expect("dispose should succeed");
    assert!(cleared_for_assertion.get());
    assert!(
        dom.context()
            .parent(&raw_node)
            .expect("parent lookup should succeed")
            .is_none()
    );
}
