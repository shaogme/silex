#![allow(unused_extern_crates)]

extern crate silex_macros_test as silex;

use silex::core::{ErrorReporter, Runtime, Rx, Scope, SilexContext, SilexResult};
use silex::css::types::Hex;
use silex::dom::attribute::{
    AttrOp, AttributeBuilder, AttributeGroup, GlobalAttributes, ReactiveBindingTarget,
};
use silex::dom::prelude::AnyView;
use silex::macros::{css, global, styled, tw, tw_variants};

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
    pub ScopedPanel<'scope><div>(
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
        color: silex::core::reactivity::Signal<'scope, Hex>,
    ) {
        color: $(color);
    }
}

styled! {
    pub ScopedSelector<'scope><div>(
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
        selector: silex::core::reactivity::Signal<'scope, String>,
    ) {
        $selector { color: red; }
    }
}

styled! {
    pub VariantPanel<'scope><div>(
        #[ctx] ctx: SilexContext<'scope>,
        children: AnyView<'scope>,
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
    pub StaticGlobal<'scope>(scope: Scope<'scope>) {
        body { color: red; }
    }
}

global! {
    pub ScopedGlobal<'scope>(
        error_handler: ErrorReporter<'scope>,
        color: silex::core::reactivity::Signal<'scope, Hex>,
        selector: silex::core::reactivity::Signal<'scope, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}

fn dynamic_width<'scope>(
    source: Rx<'scope, silex::css::types::Px>,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<silex::css::DynamicCss<'scope>> {
    css!(error_handler; width: $(source);)
}

fn dynamic_tw_width<'scope>(
    source: Rx<'scope, silex::css::types::Px>,
    error_handler: ErrorReporter<'scope>,
) -> SilexResult<silex::css::DynamicCss<'scope>> {
    tw!(error_handler; "w-[$(source)]")
}

fn conditional_class<'scope>(condition: Rx<'scope, bool>) -> AttrOp<'scope> {
    tw!(
        "inline-flex",
        (
            condition,
            "bg-blue-500 text-white",
            "bg-slate-500 text-black"
        )
    )
}

fn conditional_classes<'scope>(
    condition: silex::core::reactivity::ReadSignal<'scope, bool>,
) -> AttributeGroup<'scope> {
    silex::macros::classes!["active" => condition]
}

#[test]
fn conditional_tw_expands_to_a_scoped_attribute_operation() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, _) = scope.signal(true).unwrap();
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
        .child(|scope| {
            let (read, _) = scope.signal(silex::css::types::px(4)).unwrap();
            let error_handler = scope.error_handler(|_| {}).unwrap();
            let dynamic = dynamic_width(read.into_rx(), error_handler).unwrap();
            assert_eq!(dynamic.vars.len(), 1);
        })
        .unwrap();
}

#[test]
fn dynamic_tw_accepts_the_explicit_error_handler_syntax() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, _) = scope.signal(silex::css::types::px(4)).unwrap();
            let error_handler = scope.error_handler(|_| {}).unwrap();
            let dynamic = dynamic_tw_width(read.into_rx(), error_handler).unwrap();
            assert_eq!(dynamic.vars.len(), 1);
        })
        .unwrap();
}

#[test]
fn classes_converts_signal_to_a_scoped_attribute_group() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (condition, _) = scope.signal(true).unwrap();
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
