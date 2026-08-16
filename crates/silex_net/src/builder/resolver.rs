use std::{borrow::Cow, rc::Rc};

use silex_core::{Computed, OwnerAccess, ReadSignal, RwSignal, Rx, Signal, SilexResult};

#[cfg(feature = "persist")]
use silex_persist::Persistent;

#[derive(Clone)]
pub struct ValueResolver<'scope> {
    kind: ResolverKind<'scope>,
}

#[derive(Clone)]
enum ResolverKind<'scope> {
    Static(Cow<'static, str>),
    Dynamic {
        tracked: Rc<dyn Fn() -> SilexResult<String> + 'scope>,
        untracked: Rc<dyn Fn() -> SilexResult<String> + 'scope>,
        validate: Rc<dyn Fn(OwnerAccess<'scope>) -> SilexResult<()> + 'scope>,
    },
}

impl<'scope> ValueResolver<'scope> {
    pub fn static_value(value: impl Into<Cow<'static, str>>) -> Self {
        Self {
            kind: ResolverKind::Static(value.into()),
        }
    }

    /// Create a resolver with tracked and untracked accessors.
    pub fn dynamic<F, U>(tracked: F, untracked: U) -> Self
    where
        F: Fn() -> SilexResult<String> + 'scope,
        U: Fn() -> SilexResult<String> + 'scope,
    {
        Self {
            kind: ResolverKind::Dynamic {
                tracked: Rc::new(tracked),
                untracked: Rc::new(untracked),
                validate: Rc::new(|_| Ok(())),
            },
        }
    }

    pub(crate) fn dynamic_with_validator<F, U, V>(tracked: F, untracked: U, validate: V) -> Self
    where
        F: Fn() -> SilexResult<String> + 'scope,
        U: Fn() -> SilexResult<String> + 'scope,
        V: Fn(OwnerAccess<'scope>) -> SilexResult<()> + 'scope,
    {
        Self {
            kind: ResolverKind::Dynamic {
                tracked: Rc::new(tracked),
                untracked: Rc::new(untracked),
                validate: Rc::new(validate),
            },
        }
    }

    pub(crate) fn validate_runtime(&self, scope: OwnerAccess<'scope>) -> SilexResult<()> {
        match self {
            Self {
                kind: ResolverKind::Static(_),
                ..
            } => Ok(()),
            Self {
                kind: ResolverKind::Dynamic { validate, .. },
                ..
            } => validate(scope),
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
    ($ty:ty, $source:expr) => {
        impl<'scope, T> IntoNetValue<'scope> for $ty
        where
            T: ToString + 'scope,
        {
            fn into_net_value(self) -> ValueResolver<'scope> {
                let source: Rx<'scope, T> = ($source)(self);
                let tracked = source;
                let untracked = source;
                let validation_source = source;
                ValueResolver::dynamic_with_validator(
                    move || tracked.with(|value| value.to_string()),
                    move || untracked.with_untracked(|value| value.to_string()),
                    move |scope| scope.validate_runtime(&validation_source),
                )
            }
        }
    };
}

impl_into_net_value_for_rx!(Rx<'scope, T>, |value: Rx<'scope, T>| value);
impl_into_net_value_for_rx!(ReadSignal<'scope, T>, |value: ReadSignal<'scope, T>| {
    value.into_rx()
});
impl_into_net_value_for_rx!(RwSignal<'scope, T>, |value: RwSignal<'scope, T>| {
    value.into_rx()
});
impl_into_net_value_for_rx!(Signal<'scope, T>, |value: Signal<'scope, T>| {
    value.into_rx()
});
impl_into_net_value_for_rx!(Computed<'scope, T>, |value: Computed<'scope, T>| value
    .into_rx());

#[cfg(feature = "persist")]
impl_into_net_value_for_rx!(Persistent<'scope, T>, |value: Persistent<'scope, T>| value
    .signal()
    .into_rx());

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
        ValueResolver::dynamic(move || Ok(tracked()), move || Ok(value()))
    }
}

#[cfg(test)]
mod tests {
    use super::{IntoNetValue, ValueResolver};
    use silex_core::Runtime;

    #[test]
    fn static_and_owned_closure_resolvers_resolve_values() {
        let static_value = ValueResolver::static_value("static");
        assert_eq!(static_value.resolve().unwrap(), "static");

        let closure = (|| 42_i32).into_net_value();
        assert_eq!(closure.resolve().unwrap(), "42");
    }

    #[test]
    fn reactive_resolver_tracks_and_reads_untracked_values() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|scope| {
                let (value, set_value) = scope.signal(1_i32).unwrap();
                let resolver = value.into_net_value();

                assert_eq!(resolver.resolve_tracked().unwrap(), "1");
                set_value.set(2).unwrap();
                assert_eq!(resolver.resolve().unwrap(), "2");
            })
            .unwrap();
    }

    #[test]
    fn dynamic_resolver_can_read_multiple_sources() {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|scope| {
                let (first, _) = scope.signal("a".to_string()).unwrap();
                let (second, _) = scope.signal("b".to_string()).unwrap();
                let tracked_first = first;
                let tracked_second = second;
                let untracked_first = first;
                let untracked_second = second;
                let resolver = ValueResolver::dynamic(
                    move || Ok(format!("{}{}", tracked_first.get()?, tracked_second.get()?)),
                    move || {
                        Ok(format!(
                            "{}{}",
                            untracked_first.get_untracked()?,
                            untracked_second.get_untracked()?
                        ))
                    },
                );

                assert_eq!(resolver.resolve_tracked().unwrap(), "ab");
                assert_eq!(resolver.resolve().unwrap(), "ab");
            })
            .unwrap();
    }
}
