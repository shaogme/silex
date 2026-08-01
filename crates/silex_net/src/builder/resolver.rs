use std::{borrow::Cow, rc::Rc};

use silex_core::Rx;
use silex_core::reactivity::{Memo, ReadSignal, RwSignal, Signal};
use silex_core::traits::RxRead;

#[cfg(feature = "persist")]
use silex_persist::Persistent;

#[derive(Clone)]
pub enum ValueResolver {
    Static(Cow<'static, str>),
    Dynamic(Rc<dyn Fn() -> String>),
}

impl ValueResolver {
    pub(crate) fn resolve(&self) -> String {
        match self {
            Self::Static(value) => value.to_string(),
            Self::Dynamic(fun) => fun(),
        }
    }
}

pub trait IntoNetValue {
    fn into_net_value(self) -> ValueResolver;
}

impl IntoNetValue for ValueResolver {
    fn into_net_value(self) -> ValueResolver {
        self
    }
}

impl IntoNetValue for String {
    fn into_net_value(self) -> ValueResolver {
        ValueResolver::Static(Cow::Owned(self))
    }
}

impl IntoNetValue for &'static str {
    fn into_net_value(self) -> ValueResolver {
        ValueResolver::Static(Cow::Borrowed(self))
    }
}

macro_rules! impl_into_net_value_for_rx {
    (($($gen:tt)*) => $ty:ty) => {
        impl<$($gen)*> IntoNetValue for $ty
        where
            Self: silex_core::traits::RxRead + 'static,
            <Self as silex_core::traits::RxValue>::Value: ToString,
        {
            fn into_net_value(self) -> ValueResolver {
                ValueResolver::Dynamic(Rc::new(move || self.with(|v| v.to_string())))
            }
        }
    };
}

impl_into_net_value_for_rx!((T, M) => Rx<T, M>);
impl_into_net_value_for_rx!((T) => ReadSignal<T>);
impl_into_net_value_for_rx!((T) => RwSignal<T>);
impl_into_net_value_for_rx!((T) => Signal<T>);
impl_into_net_value_for_rx!((T) => Memo<T>);

#[cfg(feature = "persist")]
impl_into_net_value_for_rx!((T) => Persistent<T>);

macro_rules! impl_into_net_value_for_prim {
    ($($ty:ty),*) => {
        $(
            impl IntoNetValue for $ty {
                fn into_net_value(self) -> ValueResolver {
                    ValueResolver::Static(Cow::Owned(self.to_string()))
                }
            }
        )*
    };
}

impl_into_net_value_for_prim!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, bool, f32, f64
);

impl<F, T> IntoNetValue for F
where
    F: Fn() -> T + 'static,
    T: ToString,
{
    fn into_net_value(self) -> ValueResolver {
        ValueResolver::Dynamic(Rc::new(move || self().to_string()))
    }
}
