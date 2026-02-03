use super::{ApplyToDom, AttributeGroup, ReactiveApply};
use silex_core::reactivity::{Constant, Memo, ReadSignal, RwSignal, Signal};

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

// 4.1 静态条件类 (Key, bool)
impl<K: IntoStorable<Stored = String>> IntoStorable for (K, bool) {
    type Stored = (String, bool);
    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1)
    }
}

// 4.2 响应式条件类 (Key, Fn -> bool)
impl<K, F> IntoStorable for (K, F)
where
    K: IntoStorable<Stored = String>,
    F: Fn() -> bool + 'static,
{
    type Stored = (String, F);
    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1)
    }
}

// 4.3 Signal 条件类 (Key, Signal<bool>)
// 仅针对 bool 实现，因为 ApplyToDom 只实现了 (Key, Signal<bool>)

macro_rules! impl_tuple_signal {
    ($($ty:ident),*) => {
        $(
            impl<K> IntoStorable for (K, $ty<bool>)
            where
                K: IntoStorable<Stored = String>,
            {
                type Stored = (String, $ty<bool>);
                fn into_storable(self) -> Self::Stored {
                    (self.0.into_storable(), self.1)
                }
            }
        )*
    };
}

impl_tuple_signal!(ReadSignal, RwSignal, Signal, Memo, Constant);

// 4.4 Style 键值对 (Key, String-like)
// 需要小心区分 (K, V) 和下面的 (K, Signal)

// 显式实现常见组合，避免与上面的泛型冲突
// 这里主要处理 Value 是字符串的情况

impl IntoStorable for (&str, &str) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1.to_string())
    }
}

impl IntoStorable for (String, &str) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0, self.1.to_string())
    }
}

impl IntoStorable for (&str, String) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

impl IntoStorable for (String, String) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for (&str, &String) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1.clone())
    }
}

impl IntoStorable for (String, &String) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0, self.1.clone())
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
