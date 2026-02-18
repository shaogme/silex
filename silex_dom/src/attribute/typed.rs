use silex_core::reactivity::{
    Constant, Derived, Effect, Memo, ReactiveBinary, ReadSignal, RwSignal, Signal,
};
use silex_core::traits::{Get, Track, WithUntracked};
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

// Implementations for Static Types
impl ApplyStringAttribute for String {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + Clone + 'static,
    {
        setter(&self);
    }
}

// Reference types are handled by IntoStorable converting to String/Owned usually,
// but if IntoStorable returns &str or similar (it returns Stored='static), it's covered.

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
                    Effect::new(move |_| {
                        setter(&self.get().to_string());
                    });
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

// Derived
impl<S, F, U> ApplyStringAttribute for Derived<S, F>
where
    S: WithUntracked + Track + Clone + 'static,
    F: Fn(&S::Value) -> U + Clone + 'static,
    U: Display + Clone + 'static, // U must be Display + Clone
{
    fn apply_string<Set>(self, setter: Set)
    where
        Set: Fn(&str) + Clone + 'static,
    {
        Effect::new(move |_| {
            setter(&self.get().to_string());
        });
    }
}

// ReactiveBinary
impl<L, R, F, U> ApplyStringAttribute for ReactiveBinary<L, R, F>
where
    L: WithUntracked + Track + Clone + 'static,
    R: WithUntracked + Track + Clone + 'static,
    F: Fn(&L::Value, &R::Value) -> U + Clone + 'static,
    U: Display + Clone + 'static,
{
    fn apply_string<Set>(self, setter: Set)
    where
        Set: Fn(&str) + Clone + 'static,
    {
        Effect::new(move |_| {
            setter(&self.get().to_string());
        });
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
                    Effect::new(move |_| {
                        setter(self.get().into());
                    });
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

// Derived
impl<S, F, U> ApplyBoolAttribute for Derived<S, F>
where
    S: WithUntracked + Track + Clone + 'static,
    F: Fn(&S::Value) -> U + Clone + 'static,
    U: Into<bool> + Clone + 'static,
{
    fn apply_bool<Set>(self, setter: Set)
    where
        Set: Fn(bool) + Clone + 'static,
    {
        Effect::new(move |_| {
            setter(self.get().into());
        });
    }
}

// ReactiveBinary
impl<L, R, F, U> ApplyBoolAttribute for ReactiveBinary<L, R, F>
where
    L: WithUntracked + Track + Clone + 'static,
    R: WithUntracked + Track + Clone + 'static,
    F: Fn(&L::Value, &R::Value) -> U + Clone + 'static,
    U: Into<bool> + Clone + 'static,
{
    fn apply_bool<Set>(self, setter: Set)
    where
        Set: Fn(bool) + Clone + 'static,
    {
        Effect::new(move |_| {
            setter(self.get().into());
        });
    }
}
