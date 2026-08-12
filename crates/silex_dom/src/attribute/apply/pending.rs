use silex_core::SilexResult;
use std::borrow::Cow;
use std::rc::Rc;
use web_sys::Element as WebElem;

use super::foundation::{ApplyTarget, ApplyToDom};
use crate::attribute::op::{AttrOp, CombinedClasses, CombinedStyles};
use crate::view::{ViewErrorHandler, ViewOwnerToken};

// --- Attribute Forwarding Support ---

/// `PendingAttribute` 是 `AttrOp` 的零成本别名，用于统一延迟属性指令。
pub type PendingAttribute<'scope> = AttrOp<'scope>;

#[derive(Default)]
struct ClassAccumulator<'scope> {
    statics: Vec<Cow<'scope, str>>,
    toggles: Vec<(Cow<'scope, str>, silex_core::Rx<'scope, bool>)>,
    reactives: Vec<silex_core::Rx<'scope, String>>,
}

impl<'scope> ClassAccumulator<'scope> {
    fn push_static(&mut self, c: Cow<'scope, str>) {
        if !self.statics.contains(&c) {
            self.statics.push(c);
        }
    }

    fn push_toggle(&mut self, name: Cow<'scope, str>, rx: silex_core::Rx<'scope, bool>) {
        if let Some(idx) = self.toggles.iter().position(|(n, _)| n == &name) {
            self.toggles[idx] = (name, rx);
        } else {
            self.toggles.push((name, rx));
        }
    }

    fn push_reactive(&mut self, rx: silex_core::Rx<'scope, String>) {
        self.reactives.push(rx);
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
    properties: Vec<(Cow<'scope, str>, silex_core::Rx<'scope, String>)>,
    sheets: Vec<silex_core::Rx<'scope, String>>,
}

impl<'scope> StyleAccumulator<'scope> {
    fn push_static(&mut self, key: Cow<'scope, str>, val: Cow<'scope, str>) {
        if let Some(idx) = self.statics.iter().position(|(k, _)| k == &key) {
            self.statics[idx] = (key, val);
        } else {
            self.statics.push((key, val));
        }
    }

    fn push_property(&mut self, key: Cow<'scope, str>, rx: silex_core::Rx<'scope, String>) {
        if let Some(idx) = self.properties.iter().position(|(k, _)| k == &key) {
            self.properties[idx] = (key, rx);
        } else {
            self.properties.push((key, rx));
        }
    }

    fn push_sheet(&mut self, rx: silex_core::Rx<'scope, String>) {
        self.sheets.push(rx);
    }

    fn extend_combined(&mut self, combined: CombinedStyles<'scope>) {
        for (k, v) in combined.statics {
            self.push_static(k, v);
        }
        for (k, rx) in combined.properties {
            self.push_property(k, rx);
        }
        for rx in combined.sheets {
            self.push_sheet(rx);
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
            AttrOp::CustomWithInputs { .. } => {
                consolidated.push(op);
            }
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
        f: impl Fn(&WebElem, &ViewOwnerToken<'scope>, ViewErrorHandler<'scope>) -> SilexResult<()>
        + 'scope,
    ) -> Self {
        AttrOp::Custom(Rc::new(f))
    }
}
