use silex_core::reactivity::{Constant, Memo, ReadSignal, RwSignal, Signal};
use silex_core::traits::Get;
use std::fmt::Display;

// --- 9. Apply Traits (for Codegen) ---

pub trait ApplyStringAttribute {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + Clone + 'static;
}

pub trait ApplyBoolAttribute {
    fn apply_bool<F>(self, setter: F)
    where
        F: Fn(bool) + Clone + 'static;
}

// --- Internal Generic Helpers (Shared to reduce monomorphization) ---

fn apply_string_reactive_internal<F>(f: Rc<dyn Fn() -> String>, setter: F)
where
    F: Fn(&str) + Clone + 'static,
{
    silex_core::reactivity::Effect::new(move |_| {
        setter(&f());
    });
}

fn apply_bool_reactive_internal<F>(f: Rc<dyn Fn() -> bool>, setter: F)
where
    F: Fn(bool) + Clone + 'static,
{
    silex_core::reactivity::Effect::new(move |_| {
        setter(f());
    });
}

// Implementations for Static Types
impl ApplyStringAttribute for String {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + Clone + 'static,
    {
        setter(&self);
    }
}

impl ApplyBoolAttribute for bool {
    fn apply_bool<F>(self, setter: F)
    where
        F: Fn(bool) + Clone + 'static,
    {
        setter(self);
    }
}

// Primitives as Strings (for convenience)
macro_rules! impl_apply_string_primitive {
    ($($t:ty),*) => {
        $(
            impl ApplyStringAttribute for $t {
                fn apply_string<F>(self, setter: F)
                where
                    F: Fn(&str) + Clone + 'static,
                {
                    setter(&self.to_string());
                }
            }
        )*
    };
}
impl_apply_string_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

// --- Reactive Implementations ---
use std::rc::Rc;

// Macro to implement for standard signals (ReadSignal, RwSignal, Signal, Memo)
macro_rules! impl_apply_string_for_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> ApplyStringAttribute for $ty<T>
            where
                T: Display + Clone + 'static,
            {
                fn apply_string<F>(self, setter: F)
                where
                    F: Fn(&str) + Clone + 'static,
                {
                    apply_string_reactive_internal(Rc::new(move || self.get().to_string()), setter);
                }
            }
        )*
    };
}

impl_apply_string_for_signal!(ReadSignal, RwSignal, Signal, Memo);

// Constant: Optimization (No Effect needed)
impl<T> ApplyStringAttribute for Constant<T>
where
    T: Display + Clone + 'static,
{
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + Clone + 'static,
    {
        setter(&self.get().to_string());
    }
}

// --- ApplyBoolAttribute Implementations ---

// Macro for standard signals
macro_rules! impl_apply_bool_for_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> ApplyBoolAttribute for $ty<T>
            where
                T: Into<bool> + Clone + 'static,
            {
                fn apply_bool<F>(self, setter: F)
                where
                    F: Fn(bool) + Clone + 'static,
                {
                    apply_bool_reactive_internal(Rc::new(move || self.get().into()), setter);
                }
            }
        )*
    };
}

impl_apply_bool_for_signal!(ReadSignal, RwSignal, Signal, Memo);

// Constant: No Effect
impl<T> ApplyBoolAttribute for Constant<T>
where
    T: Into<bool> + Clone + 'static,
{
    fn apply_bool<F>(self, setter: F)
    where
        F: Fn(bool) + Clone + 'static,
    {
        setter(self.get().into());
    }
}
