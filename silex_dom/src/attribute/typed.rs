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
        F: Fn(&str) + Clone + 'static,
    {
        setter(&self);
    }
}

impl ApplyStringAttribute for &'static str {
    fn apply_string<F>(self, setter: F)
    where
        F: Fn(&str) + Clone + 'static,
    {
        setter(self);
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
use silex_core::traits::RxRead;
use std::rc::Rc;

impl<T> ApplyStringAttribute for silex_core::reactivity::Signal<T>
where
    T: Display + Clone + 'static,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + Clone + 'static,
    {
        apply_string_reactive_internal(Rc::new(move || self.with(|v| v.to_string())), setter);
    }
}

impl<T> ApplyBoolAttribute for silex_core::reactivity::Signal<T>
where
    T: Into<bool> + Clone + 'static,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + Clone + 'static,
    {
        apply_bool_reactive_internal(Rc::new(move || self.with(|v| v.clone().into())), setter);
    }
}

impl<V, M> ApplyStringAttribute for silex_core::Rx<V, M>
where
    Self: silex_core::traits::IntoSignal<Value = V> + 'static,
    V: Display + Clone + 'static,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + Clone + 'static,
    {
        use silex_core::traits::IntoSignal;
        self.into_signal().apply_string(setter);
    }
}

impl<V, M> ApplyBoolAttribute for silex_core::Rx<V, M>
where
    Self: silex_core::traits::IntoSignal<Value = V> + 'static,
    V: Into<bool> + Clone + 'static,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + Clone + 'static,
    {
        use silex_core::traits::IntoSignal;
        self.into_signal().apply_bool(setter);
    }
}

// 仅保留归一化终点 Signal 和用户侧外观类型 Rx。
// 其余类型由于已经被 IntoStorable 或归一化系统转换，无需独立实现。
