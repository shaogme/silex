//! Reactive-source promotion and input aggregation.

use crate::{
    ErrorReporter, Rx, RxValueKind, Scope,
    reactivity::{
        Constant, Memo, Mutation, ReadSignal, Resource, RwSignal, Signal, SignalSlice, StoredValue,
    },
    traits::{RxCloneData, RxData, RxError, RxRead, RxValue},
};
use silex_reactivity::RuntimeInputs;

/// A source description that can be materialized in a target scope after one
/// complete input validation.
pub struct PromotionPlan<'scope, T: 'scope> {
    inputs: RuntimeInputs,
    materializer: Materializer<'scope, T>,
}

type DerivedMaterializer<'scope, T> = Box<
    dyn FnOnce(
            Scope<'scope>,
            RuntimeInputs,
            ErrorReporter<'scope>,
        ) -> crate::SilexResult<Rx<'scope, T>>
        + 'scope,
>;

enum Materializer<'scope, T: 'scope> {
    Existing(Rx<'scope, T>),
    Constant(T),
    Derived(DerivedMaterializer<'scope, T>),
}

/// Source boundary for values that can be promoted into a target scope.
///
/// Implementations outside `silex_core` must return a `constant` or `derived`
/// plan. The plan's `RuntimeInputs` must include every runtime source read by
/// its materializer. The materializer must create nodes through the supplied
/// [`Scope`] and must not register nodes while building the plan.
pub trait ReactiveSource<'scope>: RxValue {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + RxData + 'scope;
}

/// Collect the opaque runtime provenance declared by a source.
pub fn runtime_inputs_of<'scope, V>(value: V) -> RuntimeInputs
where
    V: ReactiveSource<'scope>,
    V::Value: Sized + RxData + 'scope,
{
    value.into_promotion_plan().inputs()
}

impl<'scope, T: 'scope> PromotionPlan<'scope, T> {
    pub(crate) fn existing(value: Rx<'scope, T>, inputs: RuntimeInputs) -> Self {
        Self {
            inputs,
            materializer: Materializer::Existing(value),
        }
    }

    /// Create a non-reactive source plan.
    pub fn constant(value: T) -> Self {
        Self {
            inputs: RuntimeInputs::new(),
            materializer: Materializer::Constant(value),
        }
    }

    /// Create a derived source plan.
    ///
    /// `inputs` is validated against the target scope before `materializer`
    /// runs. A foreign input therefore cannot create a target node, cleanup,
    /// or initial computation run.
    pub fn derived<F>(inputs: RuntimeInputs, materializer: F) -> Self
    where
        F: FnOnce(
                Scope<'scope>,
                RuntimeInputs,
                ErrorReporter<'scope>,
            ) -> crate::SilexResult<Rx<'scope, T>>
            + 'scope,
    {
        Self {
            inputs,
            materializer: Materializer::Derived(Box::new(materializer)),
        }
    }

    pub(crate) fn inputs(&self) -> RuntimeInputs {
        self.inputs.clone()
    }

    pub(crate) fn materialize(
        self,
        scope: Scope<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> crate::SilexResult<Rx<'scope, T>> {
        scope.validate_inputs(&self.inputs)?;
        let Self {
            inputs,
            materializer,
        } = self;
        match materializer {
            Materializer::Existing(value) => Ok(value),
            Materializer::Constant(value) => scope.constant(value),
            Materializer::Derived(materializer) => materializer(scope, inputs, error_handler),
        }
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for Constant<T> {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        PromotionPlan::constant(self.into_inner())
    }
}

macro_rules! impl_primitive_sources {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<'scope> ReactiveSource<'scope> for $ty {
                fn into_promotion_plan(self) -> PromotionPlan<'scope, Self::Value>
                where
                    Self: Sized,
                    Self::Value: Sized + RxData + 'scope,
                {
                    PromotionPlan::constant(self)
                }
            }
        )*
    };
}

impl_primitive_sources!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
);

impl<'scope> ReactiveSource<'scope> for &str {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, String>
    where
        Self: Sized,
        String: Sized + RxData + 'scope,
    {
        PromotionPlan::constant(self.to_owned())
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for ReadSignal<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        let inputs = RuntimeInputs::single(self.inner.runtime_input());
        PromotionPlan::existing(self.into_rx(), inputs)
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for RwSignal<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        self.read.into_promotion_plan()
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for Signal<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        self.rx.into_promotion_plan()
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for Rx<'scope, T, RxValueKind>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        PromotionPlan::existing(self, self.runtime_inputs())
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for Memo<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        let inputs = RuntimeInputs::single(self.inner.runtime_input());
        PromotionPlan::existing(self.into_rx(), inputs)
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for StoredValue<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        let inputs = RuntimeInputs::single(self.inner.runtime_input());
        PromotionPlan::existing(self.into_rx(), inputs)
    }
}

macro_rules! impl_tuple_sources {
    ($($name:ident : $index:tt),+ $(,)?) => {
        #[allow(non_snake_case)]
        impl<'scope, $($name),+> ReactiveSource<'scope> for ($($name,)+)
        where
            $($name: ReactiveSource<'scope> + 'scope,)+
            $($name::Value: Sized + RxData + Clone + 'scope,)+
        {
            fn into_promotion_plan(self) -> PromotionPlan<'scope, Self::Value>
            where
                Self: Sized,
                Self::Value: Sized + RxData + 'scope,
            {
                $(let $name = self.$index.into_promotion_plan();)+
                let mut inputs = RuntimeInputs::new();
                $(inputs.extend(&$name.inputs());)+
                PromotionPlan::derived(inputs, move |scope, inputs, error_handler| {
                    $(let $name = $name.materialize(scope, error_handler)?;)+
                    scope
                    .derived_from(
                        inputs,
                        move || Ok(($($name.get()?,)+)),
                        error_handler,
                    )
                })
            }
        }
    };
}

impl_tuple_sources!(A: 0);
impl_tuple_sources!(A: 0, B: 1);
impl_tuple_sources!(A: 0, B: 1, C: 2);
impl_tuple_sources!(A: 0, B: 1, C: 2, D: 3);
impl_tuple_sources!(A: 0, B: 1, C: 2, D: 3, E: 4);
impl_tuple_sources!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

impl<'scope, S, F, O> ReactiveSource<'scope> for SignalSlice<S, F, O>
where
    S: ReactiveSource<'scope> + RxRead + 'scope,
    S::Value: Sized + RxData + 'scope,
    F: Fn(&S::Value) -> &O + 'scope,
    O: RxCloneData + 'scope,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, O>
    where
        Self: Sized,
        O: Sized + RxData + 'scope,
    {
        let source = self.source.into_promotion_plan();
        let inputs = source.inputs();
        let getter = self.getter;
        PromotionPlan::derived(inputs, move |scope, inputs, error_handler| {
            let source = source.materialize(scope, error_handler)?;
            scope.derived_from(
                inputs,
                move || source.with(|value| getter(value).clone()),
                error_handler,
            )
        })
    }
}

impl<'scope, T, E> ReactiveSource<'scope> for Resource<'scope, T, E>
where
    T: RxCloneData + RxData + 'static + 'scope,
    E: RxError + 'static + 'scope,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, Option<T>>
    where
        Self: Sized,
        Option<T>: Sized + RxData + 'scope,
    {
        let inputs = RuntimeInputs::single(self.state.inner.runtime_input());
        PromotionPlan::derived(inputs, move |scope, inputs, error_handler| {
            scope.derived_from(inputs, move || self.value(), error_handler)
        })
    }
}

impl<'scope, Arg, T, E> ReactiveSource<'scope> for Mutation<'scope, Arg, T, E>
where
    Arg: RxData + 'scope,
    T: RxCloneData + RxData + 'static + 'scope,
    E: RxError + 'static + 'scope,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, Option<T>>
    where
        Self: Sized,
        Option<T>: Sized + RxData + 'scope,
    {
        let inputs = RuntimeInputs::single(self.state.inner.runtime_input());
        PromotionPlan::derived(inputs, move |scope, inputs, error_handler| {
            scope.derived_from(inputs, move || self.value(), error_handler)
        })
    }
}
