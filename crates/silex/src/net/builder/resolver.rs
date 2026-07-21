use std::rc::Rc;

use silex_core::reactivity::{Memo, ReadSignal, RwSignal, Signal};
use silex_core::traits::RxGet;

#[cfg(feature = "persistence")]
use crate::persist::Persistent;

#[derive(Clone)]
pub enum ValueResolver {
    Static(String),
    Dynamic(Rc<dyn Fn() -> String>),
}

impl ValueResolver {
    pub(crate) fn resolve(&self) -> String {
        match self {
            Self::Static(value) => value.clone(),
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
        ValueResolver::Static(self)
    }
}

impl IntoNetValue for &str {
    fn into_net_value(self) -> ValueResolver {
        ValueResolver::Static(self.to_string())
    }
}

macro_rules! impl_into_net_value_for_rx {
    ($ty:ty) => {
        impl<T> IntoNetValue for $ty
        where
            T: ToString + Clone + 'static,
        {
            fn into_net_value(self) -> ValueResolver {
                ValueResolver::Dynamic(Rc::new(move || self.get().to_string()))
            }
        }
    };
}

impl_into_net_value_for_rx!(ReadSignal<T>);
impl_into_net_value_for_rx!(RwSignal<T>);
impl_into_net_value_for_rx!(Signal<T>);
impl_into_net_value_for_rx!(Memo<T>);

#[cfg(feature = "persistence")]
impl_into_net_value_for_rx!(Persistent<T>);

macro_rules! impl_into_net_value_for_prim {
    ($($ty:ty),*) => {
        $(
            impl IntoNetValue for $ty {
                fn into_net_value(self) -> ValueResolver {
                    ValueResolver::Static(self.to_string())
                }
            }
        )*
    };
}

impl_into_net_value_for_prim!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, bool, f32, f64
);
