use std::{borrow::Cow, rc::Rc};

use silex_core::{
    Memo, ReadSignal, RuntimeInputs, RwSignal, Rx, Signal, SilexResult, runtime_inputs_of,
    traits::RxRead,
};

#[cfg(feature = "persist")]
use silex_persist::Persistent;

#[derive(Clone)]
pub struct ValueResolver<'scope> {
    kind: ResolverKind<'scope>,
    inputs: RuntimeInputs,
}

#[derive(Clone)]
enum ResolverKind<'scope> {
    Static(Cow<'static, str>),
    Dynamic {
        tracked: Rc<dyn Fn() -> SilexResult<String> + 'scope>,
        untracked: Rc<dyn Fn() -> SilexResult<String> + 'scope>,
    },
}

impl<'scope> ValueResolver<'scope> {
    pub fn static_value(value: impl Into<Cow<'static, str>>) -> Self {
        Self {
            kind: ResolverKind::Static(value.into()),
            inputs: RuntimeInputs::new(),
        }
    }

    /// Create a resolver with explicit framework-known source provenance.
    ///
    /// The tracked closure is used by request-derived nodes. The untracked
    /// closure is used when a request is materialized for an owned future.
    pub fn dynamic_with_inputs<F, U>(tracked: F, untracked: U, inputs: RuntimeInputs) -> Self
    where
        F: Fn() -> SilexResult<String> + 'scope,
        U: Fn() -> SilexResult<String> + 'scope,
    {
        Self {
            kind: ResolverKind::Dynamic {
                tracked: Rc::new(tracked),
                untracked: Rc::new(untracked),
            },
            inputs,
        }
    }

    pub(crate) fn resolve(&self) -> SilexResult<String> {
        match self {
            Self {
                kind: ResolverKind::Static(value),
                ..
            } => Ok(value.to_string()),
            Self {
                kind: ResolverKind::Dynamic { untracked, .. },
                ..
            } => untracked(),
        }
    }

    pub(crate) fn resolve_tracked(&self) -> SilexResult<String> {
        match self {
            Self {
                kind: ResolverKind::Static(value),
                ..
            } => Ok(value.to_string()),
            Self {
                kind: ResolverKind::Dynamic { tracked, .. },
                ..
            } => tracked(),
        }
    }

    pub(crate) fn inputs(&self) -> RuntimeInputs {
        self.inputs.clone()
    }
}

pub trait IntoNetValue<'scope> {
    fn into_net_value(self) -> ValueResolver<'scope>;
}

impl<'scope> IntoNetValue<'scope> for ValueResolver<'scope> {
    fn into_net_value(self) -> ValueResolver<'scope> {
        self
    }
}

impl<'scope> IntoNetValue<'scope> for String {
    fn into_net_value(self) -> ValueResolver<'scope> {
        ValueResolver::static_value(Cow::Owned(self))
    }
}

impl<'scope> IntoNetValue<'scope> for &str {
    fn into_net_value(self) -> ValueResolver<'scope> {
        ValueResolver::static_value(Cow::Owned(self.to_string()))
    }
}

macro_rules! impl_into_net_value_for_rx {
    ($ty:ty) => {
        impl<'scope, T> IntoNetValue<'scope> for $ty
        where
            T: ToString + 'scope,
        {
            fn into_net_value(self) -> ValueResolver<'scope> {
                let inputs = runtime_inputs_of(self);
                let tracked = self;
                let untracked = self;
                ValueResolver::dynamic_with_inputs(
                    move || tracked.with(|value| value.to_string()),
                    move || untracked.with_untracked(|value| value.to_string()),
                    inputs,
                )
            }
        }
    };
}

impl_into_net_value_for_rx!(Rx<'scope, T>);
impl_into_net_value_for_rx!(ReadSignal<'scope, T>);
impl_into_net_value_for_rx!(RwSignal<'scope, T>);
impl_into_net_value_for_rx!(Signal<'scope, T>);
impl_into_net_value_for_rx!(Memo<'scope, T>);

#[cfg(feature = "persist")]
impl_into_net_value_for_rx!(Persistent<'scope, T>);

macro_rules! impl_into_net_value_for_prim {
    ($($ty:ty),*) => {
        $(
            impl<'scope> IntoNetValue<'scope> for $ty {
                fn into_net_value(self) -> ValueResolver<'scope> {
                    ValueResolver::static_value(Cow::Owned(self.to_string()))
                }
            }
        )*
    };
}

impl_into_net_value_for_prim!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, bool, f32, f64
);

impl<'scope, F, T> IntoNetValue<'scope> for F
where
    F: Fn() -> T + 'static,
    T: ToString,
{
    fn into_net_value(self) -> ValueResolver<'scope> {
        let value = Rc::new(move || self().to_string());
        let tracked = value.clone();
        ValueResolver::dynamic_with_inputs(
            move || Ok(tracked()),
            move || Ok(value()),
            RuntimeInputs::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{IntoNetValue, ValueResolver};
    use silex_core::{Runtime, runtime_inputs_of};

    #[test]
    fn static_and_owned_closure_resolvers_have_no_inputs() {
        let static_value = ValueResolver::static_value("static");
        assert_eq!(static_value.resolve().unwrap(), "static");
        assert!(static_value.inputs().is_empty());

        let closure = (|| 42_i32).into_net_value();
        assert_eq!(closure.resolve().unwrap(), "42");
        assert!(closure.inputs().is_empty());
    }

    #[test]
    fn reactive_resolver_tracks_and_reads_untracked_values() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let (value, set_value) = scope.signal(1_i32).unwrap();
                let resolver = value.into_net_value();

                assert_eq!(resolver.inputs().len(), 1);
                assert_eq!(resolver.resolve_tracked().unwrap(), "1");
                set_value.set(2).unwrap();
                assert_eq!(resolver.resolve().unwrap(), "2");
            })
            .unwrap();
    }

    #[test]
    fn explicit_resolver_aggregates_multiple_runtime_inputs() {
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let (first, _) = scope.signal("a".to_string()).unwrap();
                let (second, _) = scope.signal("b".to_string()).unwrap();
                let mut inputs = runtime_inputs_of(first);
                inputs.extend(&runtime_inputs_of(second));
                let tracked_first = first;
                let tracked_second = second;
                let untracked_first = first;
                let untracked_second = second;
                let resolver = ValueResolver::dynamic_with_inputs(
                    move || Ok(format!("{}{}", tracked_first.get()?, tracked_second.get()?)),
                    move || {
                        Ok(format!(
                            "{}{}",
                            untracked_first.get_untracked()?,
                            untracked_second.get_untracked()?
                        ))
                    },
                    inputs,
                );

                assert_eq!(resolver.inputs().len(), 2);
                assert_eq!(resolver.resolve_tracked().unwrap(), "ab");
                assert_eq!(resolver.resolve().unwrap(), "ab");
            })
            .unwrap();
    }
}
