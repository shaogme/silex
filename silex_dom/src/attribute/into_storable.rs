use super::{ApplyToDom, AttributeGroup, ReactiveApply};
use silex_core::reactivity::{
    Constant, Derived, Memo, ReactiveBinary, ReadSignal, RwSignal, Signal,
};
use silex_core::traits::{IntoSignal, Track, WithUntracked};

// --- IntoStorable: 允许非 'static 类型转换为可存储类型 ---

/// 将值转换为可存储的类型。
/// 对于引用类型（如 &str, &String），转换为 owned 类型（String）。
/// 对于已经是 'static 的类型，直接返回自身。
pub trait IntoStorable {
    /// 转换后的可存储类型，必须满足 ApplyToDom + 'static
    type Stored: ApplyToDom + 'static;

    /// 将自身转换为可存储类型
    fn into_storable(self) -> Self::Stored;
}

// --- 1. 基础类型 ---

impl IntoStorable for &str {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self.to_string()
    }
}

impl IntoStorable for &String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self.clone()
    }
}

impl IntoStorable for String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for bool {
    type Stored = bool;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

macro_rules! impl_into_storable_primitive {
    ($($t:ty),*) => {
        $(
            impl IntoStorable for $t {
                type Stored = $t;
                fn into_storable(self) -> Self::Stored {
                    self
                }
            }
        )*
    };
}
impl_into_storable_primitive!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64, char
);

// --- 2. 响应式类型 (使用宏避免泛型冲突) ---

macro_rules! impl_into_storable_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> IntoStorable for $ty<T>
            where
                T: ReactiveApply + Clone + 'static,
            {
                type Stored = Self;
                fn into_storable(self) -> Self::Stored {
                    self
                }
            }
        )*
    };
}

impl_into_storable_signal!(ReadSignal, RwSignal, Signal, Constant);

impl<T> IntoStorable for Memo<T>
where
    T: ReactiveApply + Clone + PartialEq + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<S, F, U> IntoStorable for Derived<S, F>
where
    S: WithUntracked + Track + Clone + 'static,
    F: Fn(&S::Value) -> U + Clone + 'static,
    U: ReactiveApply + Clone + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<L, R, F, U> IntoStorable for ReactiveBinary<L, R, F>
where
    L: WithUntracked + Track + Clone + 'static,
    R: WithUntracked + Track + Clone + 'static,
    F: Fn(&L::Value, &R::Value) -> U + Clone + 'static,
    U: ReactiveApply + Clone + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 3. 闭包类型 ---

impl<F, T> IntoStorable for F
where
    F: Fn() -> T + 'static,
    T: ReactiveApply + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 4. Tuple 实现 ---

// 统一泛型实现：(Key, Value)
// 适用于 (String, String) [Style], (String, bool) [Class], (String, Signal) 等所有情况
// 通过 ApplyToDom 的 generic impl 和 ReactiveApply::apply_pair 分发逻辑
impl<K, V> IntoStorable for (K, V)
where
    K: IntoStorable<Stored = String>,
    V: IntoSignal,
    V::Signal: 'static,
    (String, V::Signal): ApplyToDom,
{
    type Stored = (String, V::Signal);

    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_signal())
    }
}

// --- IntoStorable 实现：集合类型 ---

impl<V: IntoStorable, const N: usize> IntoStorable for [V; N]
where
    [V::Stored; N]: Default,
{
    type Stored = [V::Stored; N];
    fn into_storable(self) -> Self::Stored {
        self.map(|v| v.into_storable())
    }
}

impl<V: IntoStorable> IntoStorable for Option<V> {
    type Stored = Option<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.map(|v| v.into_storable())
    }
}

impl<V: IntoStorable> IntoStorable for Vec<V> {
    type Stored = Vec<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.into_iter().map(|v| v.into_storable()).collect()
    }
}

// --- IntoStorable 实现：AttributeGroup ---

// 为 AttributeGroup 生成 IntoStorable 实现的宏
macro_rules! impl_into_storable_for_group {
    ($($name:ident)+) => {
        impl<$($name: IntoStorable),+> IntoStorable for AttributeGroup<($($name,)+)> {
            type Stored = AttributeGroup<($($name::Stored,)+)>;
            fn into_storable(self) -> Self::Stored {
                #[allow(non_snake_case)]
                let ($($name,)+) = self.0;
                AttributeGroup(($($name.into_storable(),)+))
            }
        }
    };
}

impl_into_storable_for_group!(T1);
impl_into_storable_for_group!(T1 T2);
impl_into_storable_for_group!(T1 T2 T3);
impl_into_storable_for_group!(T1 T2 T3 T4);
impl_into_storable_for_group!(T1 T2 T3 T4 T5);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6 T7);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6 T7 T8);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11);
impl_into_storable_for_group!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12);
