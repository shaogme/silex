//! Sealed reactive-source promotion and input aggregation.

use crate::{
    Rx, RxValueKind, Scope,
    reactivity::{
        Constant, Memo, Mutation, ReadSignal, Resource, RwSignal, Signal, SignalSlice, StoredValue,
    },
    traits::{RxCloneData, RxData, RxError, RxRead, RxValue},
};
use silex_reactivity::{ReactiveResult, RuntimeInputs};

mod sealed {
    pub trait Sealed {}
}

/// A framework-owned source description that can be materialized in a target
/// scope after one complete input validation.
#[doc(hidden)]
pub struct PromotionPlan<'scope, T: 'scope> {
    inputs: RuntimeInputs,
    materializer: Materializer<'scope, T>,
}

type DerivedMaterializer<'scope, T> =
    Box<dyn FnOnce(&Scope<'scope>, RuntimeInputs) -> Rx<'scope, T> + 'scope>;

enum Materializer<'scope, T: 'scope> {
    Existing(Rx<'scope, T>),
    Constant(T),
    Derived(DerivedMaterializer<'scope, T>),
}

/// Sealed source boundary for all framework-known reactive values.
#[doc(hidden)]
pub trait ReactiveSource<'scope>: RxValue + sealed::Sealed {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + RxData + 'scope;
}

#[doc(hidden)]
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

    pub(crate) fn constant(value: T) -> Self {
        Self {
            inputs: RuntimeInputs::new(),
            materializer: Materializer::Constant(value),
        }
    }

    pub(crate) fn derived<F>(inputs: RuntimeInputs, materializer: F) -> Self
    where
        F: FnOnce(&Scope<'scope>, RuntimeInputs) -> Rx<'scope, T> + 'scope,
    {
        Self {
            inputs,
            materializer: Materializer::Derived(Box::new(materializer)),
        }
    }

    pub(crate) fn inputs(&self) -> RuntimeInputs {
        self.inputs.clone()
    }

    pub(crate) fn materialize(self, scope: &Scope<'scope>) -> ReactiveResult<Rx<'scope, T>> {
        scope.inner.try_validate_inputs(&self.inputs)?;
        Ok(self.materialize_unchecked(scope))
    }

    pub(crate) fn materialize_unchecked(self, scope: &Scope<'scope>) -> Rx<'scope, T> {
        let Self {
            inputs,
            materializer,
        } = self;
        match materializer {
            Materializer::Existing(value) => value,
            Materializer::Constant(value) => scope.constant(value),
            Materializer::Derived(materializer) => materializer(scope, inputs),
        }
    }
}

impl<T> sealed::Sealed for Constant<T> {}

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
            impl sealed::Sealed for $ty {}

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

impl sealed::Sealed for &str {}

impl<'scope> ReactiveSource<'scope> for &str {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, String>
    where
        Self: Sized,
        String: Sized + RxData + 'scope,
    {
        PromotionPlan::constant(self.to_owned())
    }
}

impl<'scope, T: 'scope> sealed::Sealed for ReadSignal<'scope, T> {}

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

impl<'scope, T: 'scope> sealed::Sealed for RwSignal<'scope, T> {}

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

impl<'scope, T: 'scope> sealed::Sealed for Signal<'scope, T> {}

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

impl<'scope, T: 'scope> sealed::Sealed for Rx<'scope, T, RxValueKind> {}

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

impl<'scope, T: 'scope> sealed::Sealed for Memo<'scope, T> {}

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

impl<'scope, T: 'scope> sealed::Sealed for StoredValue<'scope, T> {}

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
        impl<$($name),+> sealed::Sealed for ($($name,)+)
        where
            $($name: sealed::Sealed,)+
        {}

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
                PromotionPlan::derived(inputs, move |scope, inputs| {
                    $(let $name = $name.materialize_unchecked(scope);)+
                    scope.derived_from(inputs, move || {
                        ($($name.get(),)+)
                    })
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

impl<S, F, O> sealed::Sealed for SignalSlice<S, F, O> where S: sealed::Sealed {}

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
        PromotionPlan::derived(inputs, move |scope, inputs| {
            let source = source.materialize_unchecked(scope);
            scope.derived_from(inputs, move || source.with(|value| getter(value).clone()))
        })
    }
}

impl<'scope, T, E> sealed::Sealed for Resource<'scope, T, E> {}

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
        PromotionPlan::derived(inputs, move |scope, inputs| {
            scope.derived_from(inputs, move || self.value())
        })
    }
}

impl<'scope, Arg, T, E> sealed::Sealed for Mutation<'scope, Arg, T, E> {}

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
        PromotionPlan::derived(inputs, move |scope, inputs| {
            scope.derived_from(inputs, move || self.value())
        })
    }
}
