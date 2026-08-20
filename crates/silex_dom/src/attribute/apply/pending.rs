use silex_core::SilexResult;
use std::borrow::Cow;
use std::rc::Rc;
use web_sys::Element as WebElem;

use super::foundation::{ApplyTarget, ApplyToDom, ReactiveBindingPlan, ReactiveBindingTarget};
use crate::attribute::op::{AttrOp, CombinedClasses, CombinedStyles};
use crate::view::{MountErrorHandler, MountOwnerToken};

#[derive(Default)]
struct ClassAccumulator<'scope> {
    statics: Vec<Cow<'scope, str>>,
    toggles: Vec<(Cow<'scope, str>, ReactiveBindingPlan<'scope>)>,
    reactives: Vec<ReactiveBindingPlan<'scope>>,
}

impl<'scope> ClassAccumulator<'scope> {
    fn push_static(&mut self, c: Cow<'scope, str>) {
        if !self.statics.contains(&c) {
            self.statics.push(c);
        }
    }

    fn push_toggle(&mut self, name: Cow<'scope, str>, plan: ReactiveBindingPlan<'scope>) {
        if let Some(idx) = self.toggles.iter().position(|(n, _)| n == &name) {
            self.toggles[idx] = (name, plan);
        } else {
            self.toggles.push((name, plan));
        }
    }

    fn push_reactive(&mut self, plan: ReactiveBindingPlan<'scope>) {
        self.reactives.push(plan);
    }

    fn extend_combined(&mut self, combined: CombinedClasses<'scope>) {
        for s in combined.statics {
            self.push_static(s);
        }
        for (name, rx) in combined.toggles {
            self.push_toggle(name, rx);
        }
        for rx in combined.reactives {
            self.push_reactive(rx);
        }
    }

    fn is_empty(&self) -> bool {
        self.statics.is_empty() && self.toggles.is_empty() && self.reactives.is_empty()
    }

    fn into_op(self) -> AttrOp<'scope> {
        AttrOp::CombinedClasses(CombinedClasses {
            statics: self.statics,
            toggles: self.toggles,
            reactives: self.reactives,
        })
    }
}

#[derive(Default)]
struct StyleAccumulator<'scope> {
    statics: Vec<(Cow<'scope, str>, Cow<'scope, str>)>,
    properties: Vec<ReactiveBindingPlan<'scope>>,
    sheets: Vec<ReactiveBindingPlan<'scope>>,
}

impl<'scope> StyleAccumulator<'scope> {
    fn push_static(&mut self, key: Cow<'scope, str>, val: Cow<'scope, str>) {
        if let Some(idx) = self.statics.iter().position(|(k, _)| k == &key) {
            self.statics[idx] = (key, val);
        } else {
            self.statics.push((key, val));
        }
    }

    fn push_property(&mut self, plan: ReactiveBindingPlan<'scope>) {
        let key = match &plan.target {
            ReactiveBindingTarget::StyleProperty(key) => key,
            _ => return,
        };
        if let Some(idx) = self.properties.iter().position(|existing| {
            matches!(&existing.target, ReactiveBindingTarget::StyleProperty(name) if name == key)
        }) {
            self.properties[idx] = plan;
        } else {
            self.properties.push(plan);
        }
    }

    fn push_sheet(&mut self, plan: ReactiveBindingPlan<'scope>) {
        self.sheets.push(plan);
    }

    fn extend_combined(&mut self, combined: CombinedStyles<'scope>) {
        for (k, v) in combined.statics {
            self.push_static(k, v);
        }
        for plan in combined.properties {
            self.push_property(plan);
        }
        for plan in combined.sheets {
            self.push_sheet(plan);
        }
    }

    fn is_empty(&self) -> bool {
        self.statics.is_empty() && self.properties.is_empty() && self.sheets.is_empty()
    }

    fn into_op(self) -> AttrOp<'scope> {
        AttrOp::CombinedStyles(CombinedStyles {
            statics: self.statics,
            properties: self.properties,
            sheets: self.sheets,
        })
    }
}

pub fn consolidate_attributes<'scope>(attrs: Vec<AttrOp<'scope>>) -> Vec<AttrOp<'scope>> {
    if attrs.is_empty() {
        return attrs;
    }

    // 快速路径：单属性且无需合并时直接返回
    if attrs.len() == 1 {
        match &attrs[0] {
            AttrOp::Sequence(_) | AttrOp::CombinedClasses(_) | AttrOp::CombinedStyles(_) => {}
            _ => return attrs,
        }
    }

    let mut class_acc = ClassAccumulator::default();
    let mut style_acc = StyleAccumulator::default();
    let mut consolidated = Vec::with_capacity(attrs.len());

    // 递归打平函数
    fn process_op<'scope>(
        op: AttrOp<'scope>,
        class_acc: &mut ClassAccumulator<'scope>,
        style_acc: &mut StyleAccumulator<'scope>,
        consolidated: &mut Vec<AttrOp<'scope>>,
    ) {
        match op {
            AttrOp::Sequence(ops) => {
                for sub_op in ops {
                    process_op(sub_op, class_acc, style_acc, consolidated);
                }
            }
            AttrOp::CombinedClasses(cc) => {
                class_acc.extend_combined(cc);
            }
            AttrOp::CombinedStyles(cs) => {
                style_acc.extend_combined(cs);
            }
            AttrOp::Reactive(plan) => match &plan.target {
                ReactiveBindingTarget::ClassToggle(name) => {
                    class_acc.push_toggle(name.clone(), plan);
                }
                ReactiveBindingTarget::DynamicClasses => class_acc.push_reactive(plan),
                ReactiveBindingTarget::StyleProperty(_) => style_acc.push_property(plan),
                ReactiveBindingTarget::DynamicStyle => style_acc.push_sheet(plan),
                ReactiveBindingTarget::Attribute(_) | ReactiveBindingTarget::Custom => {
                    consolidated.push(AttrOp::Reactive(plan));
                }
            },
            AttrOp::Noop => {}
            op => {
                consolidated.push(op);
            }
        }
    }

    for op in attrs {
        process_op(op, &mut class_acc, &mut style_acc, &mut consolidated);
    }

    let mut result = Vec::with_capacity(consolidated.len() + 2);

    if !class_acc.is_empty() {
        result.push(class_acc.into_op());
    }

    if !style_acc.is_empty() {
        result.push(style_acc.into_op());
    }

    result.extend(consolidated);
    result
}

impl<'scope> AttrOp<'scope> {
    pub fn build<V>(value: V, target: ApplyTarget) -> Self
    where
        V: ApplyToDom<'scope> + 'scope,
    {
        value.into_op(target)
    }

    pub fn new_listener(f: impl Fn(&WebElem) -> SilexResult<()> + 'scope) -> Self {
        AttrOp::Custom(Rc::new(move |el, _, _| f(el)))
    }

    pub fn new_scoped(
        f: impl Fn(&WebElem, &MountOwnerToken<'scope>, MountErrorHandler<'scope>) -> SilexResult<()>
        + 'scope,
    ) -> Self {
        AttrOp::Custom(Rc::new(f))
    }
}
