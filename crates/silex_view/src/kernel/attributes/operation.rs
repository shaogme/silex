use super::apply_to_dom::ApplyToDom;
use super::{
    apply::{apply_classes, apply_styles, apply_update},
    binding::{CustomAttribute, ReactiveBindingPlan},
    model::{ApplyTarget, AttrPhase, AttrUpdate, CombinedClasses, CombinedStyles},
};
use crate::kernel::MountContext;
use silex_core::{Rx, SilexResult};
use silex_dom::model::DomElement;
use std::{borrow::Cow, fmt, rc::Rc};
#[derive(Clone)]
pub enum AttrOp<'scope> {
    Update(AttrUpdate<'scope>),
    CombinedClasses(CombinedClasses<'scope>),
    CombinedStyles(CombinedStyles<'scope>),
    Reactive(ReactiveBindingPlan<'scope>),
    Sequence(Vec<AttrOp<'scope>>),
    Custom {
        phase: AttrPhase,
        callback: CustomAttribute<'scope>,
    },
    Noop,
}

impl fmt::Debug for AttrOp<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Update(value) => formatter.debug_tuple("Update").field(value).finish(),
            Self::CombinedClasses(value) => formatter
                .debug_tuple("CombinedClasses")
                .field(value)
                .finish(),
            Self::CombinedStyles(value) => formatter
                .debug_tuple("CombinedStyles")
                .field(value)
                .finish(),
            Self::Reactive(value) => formatter.debug_tuple("Reactive").field(value).finish(),
            Self::Sequence(value) => formatter.debug_tuple("Sequence").field(value).finish(),
            Self::Custom { .. } => formatter.write_str("Custom(Rc<Fn>)"),
            Self::Noop => formatter.write_str("Noop"),
        }
    }
}
impl PartialEq for AttrOp<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Update(left), Self::Update(right)) => left == right,
            (Self::CombinedClasses(left), Self::CombinedClasses(right)) => left == right,
            (Self::CombinedStyles(left), Self::CombinedStyles(right)) => left == right,
            (Self::Reactive(left), Self::Reactive(right)) => left == right,
            (Self::Sequence(left), Self::Sequence(right)) => left == right,
            (
                Self::Custom {
                    phase: left_phase,
                    callback: left,
                },
                Self::Custom {
                    phase: right_phase,
                    callback: right,
                },
            ) => left_phase == right_phase && Rc::ptr_eq(left, right),
            (Self::Noop, Self::Noop) => true,
            _ => false,
        }
    }
}

impl<'scope> AttrOp<'scope> {
    pub fn build<V>(value: V, target: ApplyTarget) -> Self
    where
        V: ApplyToDom<'scope> + 'scope,
    {
        value.into_op(target)
    }

    pub fn static_class(value: Cow<'scope, str>) -> Self {
        Self::CombinedClasses(CombinedClasses::new(vec![value], Vec::new(), Vec::new()))
    }

    pub fn static_classes(values: Vec<Cow<'scope, str>>) -> Self {
        Self::CombinedClasses(CombinedClasses::new(values, Vec::new(), Vec::new()))
    }

    pub fn class_toggle(name: Cow<'scope, str>, rx: Rx<'scope, bool>) -> Self {
        Self::Reactive(ReactiveBindingPlan::class_toggle(name, rx))
    }

    pub fn reactive_classes(rx: Rx<'scope, String>) -> Self {
        Self::Reactive(ReactiveBindingPlan::dynamic_classes(rx))
    }

    pub fn static_styles(values: Vec<(Cow<'scope, str>, Cow<'scope, str>)>) -> Self {
        Self::CombinedStyles(CombinedStyles::new(values, Vec::new(), Vec::new()))
    }

    pub fn style_property(name: Cow<'scope, str>, rx: Rx<'scope, String>) -> Self {
        Self::Reactive(ReactiveBindingPlan::style_property(name, rx))
    }

    pub fn reactive_stylesheet(rx: Rx<'scope, String>) -> Self {
        Self::Reactive(ReactiveBindingPlan::dynamic_style(rx))
    }

    pub fn custom(
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::Custom {
            phase: AttrPhase::Staging,
            callback: Rc::new(callback),
        }
    }

    pub fn custom_phase(
        phase: AttrPhase,
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::Custom {
            phase,
            callback: Rc::new(callback),
        }
    }

    pub fn new_scoped(
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::custom(callback)
    }

    pub fn on_commit(
        callback: impl Fn(&DomElement, &MountContext<'scope>) -> SilexResult<()> + 'scope,
    ) -> Self {
        Self::custom_phase(AttrPhase::Commit, callback)
    }

    pub fn apply(self, element: &DomElement, context: &MountContext<'scope>) -> SilexResult<()> {
        let owner = context.owner();
        let handler = context.error_handler();
        match self {
            Self::Update(update) => {
                let (target, data) = update.into_parts();
                apply_update(element, target, data, &owner, context, handler)
            }
            Self::CombinedClasses(value) => apply_classes(element, value, &owner, context, handler),
            Self::CombinedStyles(value) => apply_styles(element, value, &owner, context, handler),
            Self::Reactive(plan) => plan.install(element, &owner, context),
            Self::Sequence(values) => {
                for value in values {
                    value.apply(element, context)?;
                }
                Ok(())
            }
            Self::Custom {
                phase: AttrPhase::Staging,
                callback,
            } => owner.with_runtime(|| callback(element, context))?,
            Self::Custom {
                phase: AttrPhase::Commit,
                callback,
            } => {
                let element = element.clone();
                let commit_context = context.clone();
                context.on_commit(move || callback(&element, &commit_context))?;
                Ok(())
            }
            Self::Noop => Ok(()),
        }
    }
}
