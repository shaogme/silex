#![allow(unused_extern_crates)]

extern crate silex_macros_test as silex;

use silex::core::{ErrorReporter, OwnerAccess, Runtime, Rx, Signal, SilexContext, SilexResult};
use silex::css::types::Hex;
use silex::macros::{css, global, styled, tw, tw_variants};
use silex_view::AnyView;
use silex_view::attribute::{AttrOp, AttributeGroup, ReactiveBindingTarget};

tw_variants! {
    pub struct NumericVariants {
        base: "block",
        variants: {
            size: {
                "1x": "p-1",
                sm: "p-2",
            }
        },
        default_variants: { size: "1x" }
    }
}

styled! {
    pub ScopedPanel<'owner><div>(
        #[ctx] ctx: SilexContext<'owner>,
        children: AnyView<'owner>,
        color: Rx<'owner, Hex>,
    ) {
        color: $(color);
    }
}

styled! {
    pub ScopedSelector<'owner><div>(
        #[ctx] ctx: SilexContext<'owner>,
        children: AnyView<'owner>,
        selector: Rx<'owner, String>,
    ) {
        $selector { color: red; }
    }
}

styled! {
    pub VariantPanel<'owner><div>(
        #[ctx] ctx: SilexContext<'owner>,
        children: AnyView<'owner>,
    ) {
        variants: {
            mode: {
                light: { color: red; },
                dark: { color: blue; },
            }
        }
    }
}

global! {
    pub StaticGlobal<'owner>(owner: OwnerAccess<'owner>) {
        body { color: red; }
    }
}

global! {
    pub ScopedGlobal<'owner>(
        error_handler: ErrorReporter<'owner>,
        color: Rx<'owner, Hex>,
        selector: Rx<'owner, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}

fn dynamic_width<'owner>(
    source: Rx<'owner, silex::css::types::Px>,
    error_handler: ErrorReporter<'owner>,
) -> SilexResult<silex::css::DynamicCss<'owner>> {
    css!(error_handler; width: $(source);)
}

fn dynamic_tw_width<'owner>(
    source: Rx<'owner, silex::css::types::Px>,
    error_handler: ErrorReporter<'owner>,
) -> SilexResult<silex::css::DynamicCss<'owner>> {
    tw!(error_handler; "w-[$(source)]")
}

fn conditional_class<'owner>(condition: Rx<'owner, bool>) -> AttrOp<'owner> {
    tw!(
        "inline-flex",
        (
            condition,
            "bg-blue-500 text-white",
            "bg-slate-500 text-black"
        )
    )
}

fn conditional_classes<'owner>(condition: Signal<'owner, bool>) -> AttributeGroup<'owner> {
    silex::macros::classes!["active" => condition]
}

#[test]
fn conditional_tw_expands_to_a_scoped_attribute_operation() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let read = owner.signal(true).unwrap();
            let operation = conditional_class(read.into_rx());

            match operation {
                AttrOp::Custom { .. } => {}
                other => panic!("expected Custom, got {other:?}"),
            }
        })
        .unwrap();
}

#[test]
fn dynamic_css_keeps_the_source_scope() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let read = owner.signal(silex::css::types::px(4)).unwrap();
            let error_handler = owner.error_handler(|_| {}).unwrap();
            let dynamic = dynamic_width(read.into_rx(), error_handler.view()).unwrap();
            assert_eq!(dynamic.vars.len(), 1);
        })
        .unwrap();
}

#[test]
fn dynamic_tw_accepts_the_explicit_error_handler_syntax() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let read = owner.signal(silex::css::types::px(4)).unwrap();
            let error_handler = owner.error_handler(|_| {}).unwrap();
            let dynamic = dynamic_tw_width(read.into_rx(), error_handler.view()).unwrap();
            assert_eq!(dynamic.vars.len(), 1);
        })
        .unwrap();
}

#[test]
fn classes_converts_signal_to_a_scoped_attribute_group() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let condition = owner.signal(true).unwrap();
            let group = conditional_classes(condition);
            assert_eq!(group.0.len(), 1);
            assert!(matches!(
                &group.0[0],
                AttrOp::Reactive(plan)
                    if matches!(&plan.target, ReactiveBindingTarget::ClassToggle(_))
            ));
        })
        .unwrap();
}

#[test]
fn numeric_tw_variant_names_are_selected_from_strings() {
    assert_eq!(
        NumericVariantsSize::try_from_str("1x"),
        Ok(NumericVariantsSize::Val1x)
    );
    assert!(NumericVariants::new().get_checked("1x").is_ok());
}
