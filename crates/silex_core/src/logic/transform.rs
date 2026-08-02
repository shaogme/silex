use crate::{
    Rx, Scope,
    reactivity::Memo,
    traits::{IntoRx, RxRead},
};

/// Create a typed derived node in an explicit scope.
pub trait Map: RxRead + Clone {
    fn map<'scope, 'run, U, F>(self, scope: &Scope<'scope, 'run>, f: F) -> Rx<'scope, 'run, U>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        Self::Value: Sized + 'run,
        U: 'run,
        F: Fn(&Self::Value) -> U + 'scope;

    fn map_fn<'scope, 'run, U>(
        self,
        scope: &Scope<'scope, 'run>,
        f: fn(&Self::Value) -> U,
    ) -> Rx<'scope, 'run, U>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        Self::Value: Sized + 'run,
        U: 'run;
}

impl<S> Map for S
where
    S: RxRead + Clone,
{
    fn map<'scope, 'run, U, F>(self, scope: &Scope<'scope, 'run>, f: F) -> Rx<'scope, 'run, U>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        Self::Value: Sized + 'run,
        U: 'run,
        F: Fn(&Self::Value) -> U + 'scope,
    {
        let source = self.into_rx(scope);
        source.map(f)
    }

    fn map_fn<'scope, 'run, U>(
        self,
        scope: &Scope<'scope, 'run>,
        f: fn(&Self::Value) -> U,
    ) -> Rx<'scope, 'run, U>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        Self::Value: Sized + 'run,
        U: 'run,
    {
        let source = self.into_rx(scope);
        source.map(f)
    }
}

pub trait Memoize: RxRead + Clone {
    fn memo<'scope, 'run>(self, scope: &Scope<'scope, 'run>) -> Memo<'scope, 'run, Self::Value>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'run;
}

impl<S> Memoize for S
where
    S: RxRead + Clone,
{
    fn memo<'scope, 'run>(self, scope: &Scope<'scope, 'run>) -> Memo<'scope, 'run, Self::Value>
    where
        Self: IntoRx<'scope, 'run> + 'scope,
        Self::Value: PartialEq + Clone + Sized + 'run,
    {
        let source = self.into_rx(scope);
        let scope = *scope;
        scope.memo(move |_| source.get())
    }
}
