use std::fmt::Display;

// --- 9. Apply Traits (for Codegen) ---

pub trait ApplyStringAttribute {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + 'static;
}

pub trait ApplyBoolAttribute {
    fn apply_bool<F>(self, setter: F)
    where
        F: Fn(bool) + 'static;
}

// --- Internal Generic Helpers (Shared to reduce monomorphization) ---

fn apply_string_reactive_internal<F>(f: Rc<dyn Fn() -> String>, setter: F)
where
    F: Fn(&str) + 'static,
{
    silex_core::reactivity::Effect::new(move |_| {
        setter(&f());
    });
}

fn apply_bool_reactive_internal<F>(f: Rc<dyn Fn() -> bool>, setter: F)
where
    F: Fn(bool) + 'static,
{
    silex_core::reactivity::Effect::new(move |_| {
        setter(f());
    });
}

// Implementations for Static Types
impl ApplyStringAttribute for String {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + 'static,
    {
        setter(&self);
    }
}

impl ApplyStringAttribute for &'static str {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + 'static,
    {
        setter(self);
    }
}

impl ApplyBoolAttribute for bool {
    fn apply_bool<F>(self, setter: F)
    where
        F: Fn(bool) + 'static,
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
                    F: Fn(&str) + 'static,
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

impl<V, M> ApplyStringAttribute for silex_core::Rx<V, M>
where
    V: Display + 'static,
    M: 'static,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + 'static,
    {
        use silex_core::traits::RxRead;
        apply_string_reactive_internal(Rc::new(move || self.with(|v| v.to_string())), setter);
    }
}

impl<V, M> ApplyBoolAttribute for silex_core::Rx<V, M>
where
    V: Into<bool> + Clone + 'static,
    M: 'static,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + 'static,
    {
        use silex_core::traits::RxRead;
        apply_bool_reactive_internal(Rc::new(move || self.with(|v| v.clone().into())), setter);
    }
}
