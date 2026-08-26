use super::{
    binding::{ReactiveBindingPlan, ReactiveBindingTarget},
    model::{CombinedClasses, CombinedStyles},
    operation::AttrOp,
};
use std::borrow::Cow;
pub fn consolidate_attributes<'scope>(attrs: Vec<AttrOp<'scope>>) -> Vec<AttrOp<'scope>> {
    #[derive(Default)]
    struct Consolidation<'scope> {
        classes: Vec<Cow<'scope, str>>,
        toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
        class_reactive: Vec<ReactiveBindingPlan<'scope>>,
        styles: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
        style_properties: Vec<ReactiveBindingPlan<'scope>>,
        style_sheets: Vec<ReactiveBindingPlan<'scope>>,
        result: Vec<AttrOp<'scope>>,
    }

    impl<'scope> Consolidation<'scope> {
        fn process(&mut self, op: AttrOp<'scope>) {
            match op {
                AttrOp::Sequence(values) => {
                    for value in values {
                        self.process(value);
                    }
                }
                AttrOp::CombinedClasses(value) => {
                    let (classes, toggles, reactives) = value.into_parts();
                    self.classes.extend(classes);
                    self.toggles.extend(toggles);
                    self.class_reactive.extend(reactives);
                }
                AttrOp::CombinedStyles(value) => {
                    let (styles, properties, sheets) = value.into_parts();
                    self.styles.extend(styles);
                    self.style_properties.extend(properties);
                    self.style_sheets.extend(sheets);
                }
                AttrOp::Reactive(plan) => match &plan.target {
                    ReactiveBindingTarget::ClassToggle(name) => {
                        self.toggles.push((name.clone(), plan));
                    }
                    ReactiveBindingTarget::DynamicClasses => self.class_reactive.push(plan),
                    ReactiveBindingTarget::StyleProperty(_) => self.style_properties.push(plan),
                    ReactiveBindingTarget::DynamicStyle => self.style_sheets.push(plan),
                    _ => self.result.push(AttrOp::Reactive(plan)),
                },
                AttrOp::Noop => {}
                other => self.result.push(other),
            }
        }

        fn finish(self) -> Vec<AttrOp<'scope>> {
            let Self {
                classes,
                toggles,
                class_reactive,
                styles,
                style_properties,
                style_sheets,
                mut result,
            } = self;
            if !classes.is_empty() || !toggles.is_empty() || !class_reactive.is_empty() {
                result.insert(
                    0,
                    AttrOp::CombinedClasses(CombinedClasses::new(classes, toggles, class_reactive)),
                );
            }
            if !styles.is_empty() || !style_properties.is_empty() || !style_sheets.is_empty() {
                result.insert(
                    usize::from(!result.is_empty()),
                    AttrOp::CombinedStyles(CombinedStyles::new(
                        styles,
                        style_properties,
                        style_sheets,
                    )),
                );
            }
            result
        }
    }

    let mut consolidation = Consolidation::default();
    for attr in attrs {
        consolidation.process(attr);
    }
    consolidation.finish()
}
