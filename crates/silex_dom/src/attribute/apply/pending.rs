use std::borrow::Cow;
use std::rc::Rc;
use web_sys::Element as WebElem;

use super::foundation::{ApplyTarget, ApplyToDom};
use crate::attribute::op::{
    Attr, AttrData, AttrOp, AttrTarget, AttrUpdate, ClassToggle, CombinedClasses, CombinedStyles,
    StyleProperty, parse_style_str,
};

// --- Attribute Forwarding Support ---

#[derive(Clone, PartialEq)]
pub struct PendingAttribute {
    pub op: AttrOp,
}

pub fn consolidate_attributes(attrs: Vec<PendingAttribute>) -> Vec<PendingAttribute> {
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

    let mut flattened = Vec::new();
    for attr in attrs {
        flatten_ops(attr.op, &mut flattened);
    }

    for op in flattened {
        match op {
            // --- Class 指令收集 ---
            AttrOp::SetStaticClasses(v) => {
                static_classes.extend(v);
            }
            AttrOp::AddClassToggle(ClassToggle { name, rx }) => {
                class_toggles.push((name, rx));
            }
            AttrOp::AddReactiveClasses(rx) => {
                reactive_classes.push(rx);
            }

            // --- Style 指令收集 ---
            AttrOp::SetStaticStyles(v) => {
                static_styles.extend(v);
            }
            AttrOp::BindStyleProperty(StyleProperty { name, rx }) => {
                style_props.push((name, rx));
            }
            AttrOp::BindReactiveStyleSheet(rx) => {
                style_sheets.push(rx);
            }

            // --- 通用属性指令 (检查是否为 class/style) ---
            AttrOp::Update(AttrUpdate {
                name,
                target: AttrTarget::Attr,
                data: AttrData::StaticAttr(Attr::String(value)),
            }) => {
                if name == "class" {
                    match value {
                        Cow::Borrowed(s) => {
                            for token in s.split_whitespace() {
                                static_classes.push(Cow::Borrowed(token));
                            }
                        }
                        Cow::Owned(s) => {
                            for token in s.split_whitespace() {
                                static_classes.push(Cow::Owned(token.to_string()));
                            }
                        }
                    }
                } else if name == "style" {
                    match value {
                        Cow::Borrowed(s) => {
                            for (k, v) in parse_style_str(s) {
                                static_styles.push((k, v));
                            }
                        }
                        Cow::Owned(ref s) => {
                            for (k, v) in parse_style_str(s) {
                                static_styles.push((k.into_owned().into(), v.into_owned().into()));
                            }
                        }
                    }
                } else {
                    let attr = Attr::String(value);
                    consolidated.push(PendingAttribute {
                        op: AttrOp::Update(AttrUpdate {
                            name,
                            target: AttrTarget::Attr,
                            data: AttrData::StaticAttr(attr),
                        }),
                    });
                }
            }

            // --- 合并指令收集 (防止重复合并导致覆盖) ---
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

    // 静态类名去重逻辑，减少重复 DOM class_list 操作
    if static_classes.len() > 1 {
        static_classes.dedup();
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
