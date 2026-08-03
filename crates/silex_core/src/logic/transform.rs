use crate::{
    Rx, Scope,
    reactivity::Memo,
    traits::{IntoRx, RxRead},
};

/// Create a typed derived node in an explicit scope.
pub trait Map: RxRead + Clone {
    fn map<'scope, U, F>(self, scope: &Scope<'scope>, f: F) -> Rx<'scope, U>
    where
        Self: IntoRx<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        F: Fn(&Self::Value) -> U + 'scope;

    fn map_fn<'scope, U>(self, scope: &Scope<'scope>, f: fn(&Self::Value) -> U) -> Rx<'scope, U>
    where
        Self: IntoRx<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope;
}

impl<S> Map for S
where
    S: RxRead + Clone,
{
    fn map<'scope, U, F>(self, scope: &Scope<'scope>, f: F) -> Rx<'scope, U>
    where
        Self: IntoRx<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
        F: Fn(&Self::Value) -> U + 'scope,
    {
        let source = self.into_rx(scope);
        source.map(f)
    }

    fn map_fn<'scope, U>(self, scope: &Scope<'scope>, f: fn(&Self::Value) -> U) -> Rx<'scope, U>
    where
        Self: IntoRx<'scope> + 'scope,
        Self::Value: Sized + 'scope,
        U: 'scope,
    {
        let source = self.into_rx(scope);
        source.map(f)
    }
}

pub trait Memoize: RxRead + Clone {
    fn memo<'scope>(self, scope: &Scope<'scope>) -> Memo<'scope, Self::Value>
    where
        Self: IntoRx<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope;
}

impl<S> Memoize for S
where
    S: RxRead + Clone,
{
    fn memo<'scope>(self, scope: &Scope<'scope>) -> Memo<'scope, Self::Value>
    where
        Self: IntoRx<'scope> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'scope,
    {
        let source = self.into_rx(scope);
        let scope = *scope;
        scope.memo(move |_| source.get())
    }
}
