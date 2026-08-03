use crate::{
    Rx, Scope,
    traits::{IntoRx, RxRead},
};

pub trait ReactivePartialEq: RxRead + Clone {
    fn equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope;

    fn not_equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope;
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone,
{
    fn equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left == right)))
    }

    fn not_equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialEq + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left != right)))
    }
}

pub trait ReactivePartialOrd: RxRead + Clone {
    fn greater_than<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn less_than<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn greater_than_or_equals<'scope, O>(
        &self,
        scope: &Scope<'scope>,
        other: O,
    ) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;

    fn less_than_or_equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope;
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone,
{
    fn greater_than<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left > right)))
    }

    fn less_than<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left < right)))
    }

    fn greater_than_or_equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left >= right)))
    }

    fn less_than_or_equals<'scope, O>(&self, scope: &Scope<'scope>, other: O) -> Rx<'scope, bool>
    where
        Self: IntoRx<'scope> + 'scope,
        O: IntoRx<'scope, Value = Self::Value> + 'scope,
        Self::Value: PartialOrd + Sized + 'scope,
    {
        let left = self.clone().into_rx(scope);
        let right = other.into_rx(scope);
        let scope = *scope;
        scope.derived(move || left.with(|left| right.with(|right| left <= right)))
    }
}
