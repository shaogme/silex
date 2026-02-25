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

impl<F, M> ApplyStringAttribute for silex_core::Rx<F, M>
where
    Self: RxRead + Clone + 'static,
    <Self as silex_core::traits::RxValue>::Value: Display + Clone + 'static,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + Clone + 'static,
    {
        apply_string_reactive_internal(Rc::new(move || self.with(|v| v.to_string())), setter);
    }
}

// 统一转发宏：将响应式原语转发给归一化的 Rx 处理
macro_rules! impl_reactive_typed_forwarder {
    ($($ty:ident),*) => {
        $(
            impl<T> ApplyStringAttribute for silex_core::reactivity::$ty<T>
            where
                T: Display + Clone + 'static,
                Self: silex_core::traits::IntoRx + 'static,
                <Self as silex_core::traits::IntoRx>::RxType: ApplyStringAttribute,
            {
                fn apply_string<Setter>(self, setter: Setter)
                where
                    Setter: Fn(&str) + Clone + 'static,
                {
                    use silex_core::traits::IntoRx;
                    self.into_rx().apply_string(setter)
                }
            }

            impl<T> ApplyBoolAttribute for silex_core::reactivity::$ty<T>
            where
                T: Into<bool> + Clone + 'static,
                Self: silex_core::traits::IntoRx + 'static,
                <Self as silex_core::traits::IntoRx>::RxType: ApplyBoolAttribute,
            {
                fn apply_bool<Setter>(self, setter: Setter)
                where
                    Setter: Fn(bool) + Clone + 'static,
                {
                    use silex_core::traits::IntoRx;
                    self.into_rx().apply_bool(setter)
                }
            }
        )*
    };
}

impl_reactive_typed_forwarder!(Signal, ReadSignal, RwSignal, Constant, Memo);

// 处理派生与组合类型
impl<S, F> ApplyStringAttribute for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyStringAttribute,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + Clone + 'static,
    {
        use silex_core::traits::IntoRx;
        self.into_rx().apply_string(setter)
    }
}

impl<U, const N: usize> ApplyStringAttribute for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyStringAttribute,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + Clone + 'static,
    {
        use silex_core::traits::IntoRx;
        self.into_rx().apply_string(setter)
    }
}

impl<S, F, O> ApplyStringAttribute for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyStringAttribute,
{
    fn apply_string<Setter>(self, setter: Setter)
    where
        Setter: Fn(&str) + Clone + 'static,
    {
        use silex_core::traits::IntoRx;
        self.into_rx().apply_string(setter)
    }
}

// --- ApplyBoolAttribute Implementations ---

impl<F, M> ApplyBoolAttribute for silex_core::Rx<F, M>
where
    Self: RxRead + Clone + 'static,
    <Self as silex_core::traits::RxValue>::Value: Into<bool> + Clone + 'static,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + Clone + 'static,
    {
        apply_bool_reactive_internal(Rc::new(move || self.with(|v| v.clone().into())), setter);
    }
}

impl<S, F> ApplyBoolAttribute for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyBoolAttribute,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + Clone + 'static,
    {
        use silex_core::traits::IntoRx;
        self.into_rx().apply_bool(setter)
    }
}

impl<U, const N: usize> ApplyBoolAttribute for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyBoolAttribute,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + Clone + 'static,
    {
        use silex_core::traits::IntoRx;
        self.into_rx().apply_bool(setter)
    }
}

impl<S, F, O> ApplyBoolAttribute for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx + 'static,
    <Self as silex_core::traits::IntoRx>::RxType: ApplyBoolAttribute,
{
    fn apply_bool<Setter>(self, setter: Setter)
    where
        Setter: Fn(bool) + Clone + 'static,
    {
        use silex_core::traits::IntoRx;
        self.into_rx().apply_bool(setter)
    }
}
