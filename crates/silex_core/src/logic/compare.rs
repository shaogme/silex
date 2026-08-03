use crate::{
    Rx, Scope,
    traits::{IntoRx, RxRead},
};

pub trait ReactivePartialEq: RxRead + Clone {
    fn equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope;

    fn not_equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope;
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone,
{
    fn equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left == right)))
    }

    fn not_equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left != right)))
    }
}

pub trait ReactivePartialOrd: RxRead + Clone {
    fn greater_than<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn less_than<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn greater_than_or_equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn less_than_or_equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone,
{
    fn greater_than<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left > right)))
    }

    fn less_than<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left < right)))
    }

    fn greater_than_or_equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left >= right)))
    }

    fn less_than_or_equals<'scope, 'run, O>(
        &self,
        scope: &Scope<'scope, 'run>,
        other: O,
    ) -> Rx<'scope, 'run, bool>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        O: IntoRx<'scope, 'run, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left <= right)))
    }
}
