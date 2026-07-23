use std::borrow::Cow;
use std::rc::Rc;
use web_sys::Element as WebElem;

use super::foundation::{ApplyTarget, ApplyToDom};
use crate::attribute::op::{
    AttrOp, CombinedClasses, CombinedStyles,
};

// --- Attribute Forwarding Support ---

#[derive(Clone, PartialEq)]
pub struct PendingAttribute {
    pub op: AttrOp,
}

pub fn consolidate_attributes(attrs: Vec<PendingAttribute>) -> Vec<PendingAttribute> {
    if attrs.is_empty() {
        return attrs;
    }

    // 快速路径：单属性且无需合并时直接返回
    if attrs.len() == 1 {
        match &attrs[0].op {
            AttrOp::Sequence(_) | AttrOp::CombinedClasses(_) | AttrOp::CombinedStyles(_) => {}
            _ => return attrs,
        }
    }

    let mut consolidated = Vec::new();

    // Class 收集器
    let mut static_classes: Vec<Cow<'static, str>> = Vec::new();
    let mut class_toggles: Vec<(Cow<'static, str>, silex_core::Rx<bool>)> = Vec::new();
    let mut reactive_classes: Vec<silex_core::Rx<String>> = Vec::new();

    // Style 收集器
    let mut static_styles: Vec<(Cow<'static, str>, Cow<'static, str>)> = Vec::new();
    let mut style_props: Vec<(Cow<'static, str>, silex_core::Rx<String>)> = Vec::new();
    let mut style_sheets: Vec<silex_core::Rx<String>> = Vec::new();

    // 递归打平函数
    fn flatten_ops(op: AttrOp, acc: &mut Vec<AttrOp>) {
        match op {
            AttrOp::Sequence(ops) => {
                for sub_op in ops {
                    flatten_ops(sub_op, acc);
                }
            }
            AttrOp::Noop => {}
            _ => acc.push(op),
        }
    }

    let mut flattened = Vec::with_capacity(attrs.len());
    for attr in attrs {
        flatten_ops(attr.op, &mut flattened);
    }

    for op in flattened {
        match op {
            // --- 合并指令收集 ---
            AttrOp::CombinedClasses(CombinedClasses {
                statics,
                toggles,
                reactives,
            }) => {
                static_classes.extend(statics);
                class_toggles.extend(toggles);
                reactive_classes.extend(reactives);
            }
            AttrOp::CombinedStyles(CombinedStyles {
                statics,
                properties,
                sheets,
            }) => {
                static_styles.extend(statics);
                style_props.extend(properties);
                style_sheets.extend(sheets);
            }

            // --- 其它指令，原样保留 ---
            op => {
                consolidated.push(PendingAttribute { op });
            }
        }
    }

    let mut result = Vec::with_capacity(consolidated.len() + 2);

    // 静态类名去重逻辑，保持顺序剔除重复项，减少重复 DOM class_list 操作
    if static_classes.len() > 1 {
        let mut seen = std::collections::HashSet::with_capacity(static_classes.len());
        static_classes.retain(|c| seen.insert(c.clone()));
    }

    // 静态样式去重逻辑，按 key 保留最后覆盖项
    if static_styles.len() > 1 {
        let mut seen_keys = std::collections::HashSet::with_capacity(static_styles.len());
        let mut deduplicated = Vec::with_capacity(static_styles.len());
        for (k, v) in static_styles.into_iter().rev() {
            if seen_keys.insert(k.clone()) {
                deduplicated.push((k, v));
            }
        }
        deduplicated.reverse();
        static_styles = deduplicated;
    }

    // 按需生成合并后的 Class 指令
    if !static_classes.is_empty() || !class_toggles.is_empty() || !reactive_classes.is_empty() {
        result.push(PendingAttribute {
            op: AttrOp::CombinedClasses(CombinedClasses {
                statics: static_classes,
                toggles: class_toggles,
                reactives: reactive_classes,
            }),
        });
    }

    // 按需生成合并后的 Style 指令
    if !static_styles.is_empty() || !style_props.is_empty() || !style_sheets.is_empty() {
        result.push(PendingAttribute {
            op: AttrOp::CombinedStyles(CombinedStyles {
                statics: static_styles,
                properties: style_props,
                sheets: style_sheets,
            }),
        });
    }

    result.extend(consolidated);
    result
}

impl ApplyToDom for PendingAttribute {
    fn apply(&self, el: &WebElem, _target: ApplyTarget) {
        self.apply(el);
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp {
        self.op
    }
}

impl PendingAttribute {
    pub fn build<V>(value: V, target: ApplyTarget) -> Self
    where
        V: ApplyToDom + 'static,
    {
        let op = value.into_op(target);
        Self { op }
    }

    pub fn apply(&self, el: &WebElem) {
        self.op.clone().apply(el);
    }

    pub fn new_listener(f: impl Fn(&WebElem) + 'static) -> Self {
        Self {
            op: AttrOp::Custom(Rc::new(f)),
        }
    }
}
