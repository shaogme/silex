//! Reactive-source promotion.

use crate::{
    ErrorReporter, OwnerAccess, Rx,
    reactivity::{
        Computed, Constant, Mutation, ReadSignal, Resource, Signal, SignalSlice, StoredValue,
    },
    traits::{RxCloneData, RxData, RxError, RxGet, RxRead, RxValue},
};

/// A source description that can be materialized in a target scope after one
/// complete input validation.
pub struct PromotionPlan<'scope, T: 'scope> {
    materializer: Materializer<'scope, T>,
}

type ComputedMaterializer<'owner, T> = Box<
    dyn FnOnce(OwnerAccess<'owner>, ErrorReporter<'owner>) -> crate::SilexResult<Rx<'owner, T>>
        + 'owner,
>;

enum Materializer<'scope, T: 'scope> {
    Existing(Rx<'scope, T>),
    Constant(T),
    Computed(ComputedMaterializer<'scope, T>),
}

/// Source boundary for values that can be promoted into a target scope.
///
/// Implementations outside `silex_core` must return a `constant` or `derived`
/// plan. The materializer must create nodes through the supplied [`OwnerAccess`] and
/// must not register nodes while building the plan. Ordinary reads inside the
/// materialized computation establish dependencies automatically.
pub trait ReactiveSource<'scope>: RxValue {
    fn into_promotion_plan(self) -> PromotionPlan<'scope, Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + RxData + 'scope;
}

impl<'scope, T: 'scope> PromotionPlan<'scope, T> {
    pub(crate) fn existing(value: Rx<'scope, T>) -> Self {
        Self {
            materializer: Materializer::Existing(value),
        }
    }

    /// Create a non-reactive source plan.
    pub fn constant(value: T) -> Self {
        Self {
            materializer: Materializer::Constant(value),
        }
    }

    /// Create a derived source plan.
    pub fn derived<F>(materializer: F) -> Self
    where
        F: FnOnce(OwnerAccess<'scope>, ErrorReporter<'scope>) -> crate::SilexResult<Rx<'scope, T>>
            + 'scope,
    {
        Self {
            materializer: Materializer::Computed(Box::new(materializer)),
        }
    }

    pub(crate) fn materialize(
        self,
        owner: OwnerAccess<'scope>,
        error_handler: ErrorReporter<'scope>,
    ) -> crate::SilexResult<Rx<'scope, T>> {
        let Self { materializer } = self;
        match materializer {
            Materializer::Existing(value) => Ok(value),
            Materializer::Constant(value) => owner.constant(value),
            Materializer::Computed(materializer) => materializer(owner, error_handler),
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
        PromotionPlan::existing(self.into_rx())
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
        self.read.into_promotion_plan()
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for Rx<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        PromotionPlan::existing(self)
    }
}

impl<'scope, T: 'scope> ReactiveSource<'scope> for Computed<'scope, T>
where
    T: Sized + RxData,
{
    fn into_promotion_plan(self) -> PromotionPlan<'scope, T>
    where
        Self: Sized,
        T: Sized + RxData + 'scope,
    {
        PromotionPlan::existing(self.into_rx())
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
        PromotionPlan::existing(self.into_rx())
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
                PromotionPlan::derived(move |owner, error_handler| {
                    $(let $name = $name.materialize(owner, error_handler)?;)+
                    owner.computed_always(
                        move || Ok(($($name.get()?,)+)),
                        error_handler,
                    )
                    .map(crate::Computed::into_rx)
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
        let getter = self.getter;
        PromotionPlan::derived(move |owner, error_handler| {
            let source = source.materialize(owner, error_handler)?;
            owner
                .computed_always(
                    move || source.with(|value| getter(value).clone()),
                    error_handler,
                )
                .map(crate::Computed::into_rx)
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
        PromotionPlan::derived(move |owner, error_handler| {
            owner
                .computed_always(move || self.value(), error_handler)
                .map(crate::Computed::into_rx)
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
        PromotionPlan::derived(move |owner, error_handler| {
            owner
                .computed_always(move || self.value(), error_handler)
                .map(crate::Computed::into_rx)
        })
    }
}
