use super::{
    binding::{ReactiveBinding, ReactiveBindingContext},
    dom::{apply_attr_target, parse_style, set_classes},
    model::{ApplyTarget, Attr, AttrData, AttrUpdate},
    operation::AttrOp,
    storage::AttributeGroup,
};
use crate::kernel::MountContext;
use silex_core::{Rx, SilexResult};
use silex_dom::model::DomElement;
use std::{borrow::Cow, collections::HashSet};
pub trait ApplyToDom<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()>;
    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope>
    where
        Self: Sized + 'scope,
    {
        AttrOp::custom(move |element, context| self.apply(element, target.clone(), context))
    }
}

impl<'scope> ApplyToDom<'scope> for AttrOp<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        _target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        self.clone().apply(element, context)
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        self
    }
}
impl<'scope> ApplyToDom<'scope> for AttributeGroup<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        _target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        for op in self.as_ops() {
            op.clone().apply(element, context)?;
        }
        Ok(())
    }

    fn into_op(self, _target: ApplyTarget) -> AttrOp<'scope> {
        if self.as_ops().is_empty() {
            AttrOp::Noop
        } else {
            AttrOp::Sequence(self.into_ops())
        }
    }
}
impl<'scope, 'a: 'scope> ApplyToDom<'scope> for &'a str {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, &Attr::from(*self))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match target {
            ApplyTarget::Class => {
                AttrOp::static_classes(self.split_whitespace().map(Cow::Borrowed).collect())
            }
            ApplyTarget::Style => AttrOp::static_styles(
                parse_style(self)
                    .map(|(key, value)| (Cow::Owned(key.into()), Cow::Owned(value.into())))
                    .collect(),
            ),
            target => AttrOp::Update(AttrUpdate::new(
                target,
                AttrData::StaticAttr(Attr::from(self)),
            )),
        }
    }
}
impl<'scope> ApplyToDom<'scope> for String {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, &Attr::from(self.clone()))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Update(AttrUpdate::new(
            target,
            AttrData::StaticAttr(Attr::from(self)),
        ))
    }
}
impl<'scope> ApplyToDom<'scope> for &String {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(
            context.dom(),
            element,
            &target,
            &Attr::from((*self).clone()),
        )
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        self.to_string().into_op(target)
    }
}
impl<'scope, 'a: 'scope> ApplyToDom<'scope> for Cow<'a, str> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(
            context.dom(),
            element,
            &target,
            &Attr::from(self.to_string()),
        )
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        match self {
            Cow::Borrowed(value) => value.into_op(target),
            Cow::Owned(value) => value.into_op(target),
        }
    }
}
impl<'scope> ApplyToDom<'scope> for Attr<'scope> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, self)
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Update(AttrUpdate::new(target, AttrData::StaticAttr(self)))
    }
}
impl<'scope> ApplyToDom<'scope> for bool {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        apply_attr_target(context.dom(), element, &target, &Attr::from(*self))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Update(AttrUpdate::new(
            target,
            AttrData::StaticAttr(Attr::from(self)),
        ))
    }
}

macro_rules! impl_static_apply { ($($ty:ty),*) => { $(impl<'scope> ApplyToDom<'scope> for $ty { fn apply(&self, element: &DomElement, target: ApplyTarget, context: &MountContext<'scope>) -> SilexResult<()> { let value = self.to_string(); value.apply(element, target, context) } fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> { self.to_string().into_op(target) } })* }; }
impl_static_apply!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

impl<'scope, T> ApplyToDom<'scope> for Rx<'scope, T>
where
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        if let Some(plan) = T::binding_plan(*self, ReactiveBindingContext::Value(target)) {
            plan.install(element, &context.owner(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        T::binding_plan(self, ReactiveBindingContext::Value(target))
            .map_or(AttrOp::Noop, AttrOp::Reactive)
    }
}

impl<'scope, V: ApplyToDom<'scope> + 'scope> ApplyToDom<'scope> for Option<V> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        if let Some(value) = self {
            value.apply(element, target, context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        self.map_or(AttrOp::Noop, |value| value.into_op(target))
    }
}
impl<'scope, V: ApplyToDom<'scope> + 'scope> ApplyToDom<'scope> for Vec<V> {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        for value in self {
            value.apply(element, target.clone(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Sequence(
            self.into_iter()
                .map(|value| value.into_op(target.clone()))
                .collect(),
        )
    }
}
impl<'scope, V: ApplyToDom<'scope> + 'scope, const N: usize> ApplyToDom<'scope> for [V; N] {
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        for value in self {
            value.apply(element, target.clone(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        AttrOp::Sequence(
            self.into_iter()
                .map(|value| value.into_op(target.clone()))
                .collect(),
        )
    }
}

impl<'scope, K, T> ApplyToDom<'scope> for (K, Rx<'scope, T>)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
    T: ReactiveBinding<'scope> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key = self.0.clone().into();
        if let Some(plan) = T::binding_plan(self.1, ReactiveBindingContext::Pair { key, target }) {
            plan.install(element, &context.owner(), context)?;
        }
        Ok(())
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        T::binding_plan(
            self.1,
            ReactiveBindingContext::Pair {
                key: self.0.into(),
                target,
            },
        )
        .map_or(AttrOp::Noop, AttrOp::Reactive)
    }
}
impl<'scope, K> ApplyToDom<'scope> for (K, String)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key = self.0.clone().into();
        let target = if target == ApplyTarget::Apply {
            ApplyTarget::attr(key)
        } else {
            target
        };
        apply_attr_target(context.dom(), element, &target, &Attr::from(self.1.clone()))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let key: Cow<'static, str> = self.0.into();
        let target = if target == ApplyTarget::Apply {
            ApplyTarget::attr(key.clone())
        } else {
            target
        };
        match target {
            ApplyTarget::Style => AttrOp::static_styles(vec![(key, Cow::Owned(self.1))]),
            ApplyTarget::Class => AttrOp::static_class(Cow::Owned(self.1)),
            target => AttrOp::Update(AttrUpdate::new(
                target,
                AttrData::StaticAttr(Attr::from(self.1)),
            )),
        }
    }
}
impl<'scope, K> ApplyToDom<'scope> for (K, &'static str)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key: Cow<'static, str> = self.0.clone().into();
        let target = if target == ApplyTarget::Apply {
            ApplyTarget::attr(key)
        } else {
            target
        };
        apply_attr_target(context.dom(), element, &target, &Attr::from(self.1))
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        (self.0, self.1.to_string()).into_op(target)
    }
}
impl<'scope, K> ApplyToDom<'scope> for (K, bool)
where
    K: Into<Cow<'static, str>> + Clone + 'scope,
{
    fn apply(
        &self,
        element: &DomElement,
        target: ApplyTarget,
        context: &MountContext<'scope>,
    ) -> SilexResult<()> {
        let key: Cow<'static, str> = self.0.clone().into();
        if target == ApplyTarget::Class {
            let mut classes = HashSet::new();
            if self.1 {
                classes.insert(key.to_string());
            }
            set_classes(context.dom(), element, &classes)
        } else {
            let target = if target == ApplyTarget::Apply {
                ApplyTarget::attr(key)
            } else {
                target
            };
            apply_attr_target(context.dom(), element, &target, &Attr::from(self.1))
        }
    }

    fn into_op(self, target: ApplyTarget) -> AttrOp<'scope> {
        let key: Cow<'static, str> = self.0.into();
        if target == ApplyTarget::Class {
            if self.1 {
                AttrOp::static_class(key)
            } else {
                AttrOp::Noop
            }
        } else {
            AttrOp::Update(AttrUpdate::new(
                if target == ApplyTarget::Apply {
                    ApplyTarget::attr(key)
                } else {
                    target
                },
                AttrData::StaticAttr(Attr::from(self.1)),
            ))
        }
    }
}
