use super::{ApplyToDom, AttributeGroup, ReactiveApply};
use silex_core::reactivity::{Memo, ReadSignal, RwSignal, Signal};

// --- IntoStorable: 允许非 'static 类型转换为可存储类型 ---

/// 将值转换为可存储的类型。
/// 对于引用类型（如 &str, &String），转换为 owned 类型（String）。
/// 对于已经是 'static 的类型，直接返回自身。
/// 这允许用户传入 &str 和 &String 而不需要 'static 约束。
pub trait IntoStorable {
    /// 转换后的可存储类型，必须满足 ApplyToDom + 'static
    type Stored: ApplyToDom + 'static;

    /// 将自身转换为可存储类型
    fn into_storable(self) -> Self::Stored;
}

// --- IntoStorable 实现：字符串类型 ---

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

// --- IntoStorable 实现：bool ---

impl IntoStorable for bool {
    type Stored = bool;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- IntoStorable 实现：响应式类型 ---
// 这些类型本身已经是 'static，所以直接返回自身

impl<T> IntoStorable for ReadSignal<T>
where
    T: ReactiveApply + Clone + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<T> IntoStorable for Memo<T>
where
    T: ReactiveApply + Clone + PartialEq + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<T> IntoStorable for RwSignal<T>
where
    T: ReactiveApply + Clone + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<T> IntoStorable for Signal<T>
where
    T: ReactiveApply + Clone + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- IntoStorable 实现：闭包类型 ---
// 闭包返回值需要实现 ReactiveApply + 'static

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

// --- IntoStorable 实现：元组类型（用于条件类等） ---

// (String, bool) 用于静态条件类
impl IntoStorable for (String, bool) {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// (&str, bool) 用于静态条件类 - 转换为 (String, bool)
impl IntoStorable for (&str, bool) {
    type Stored = (String, bool);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

// (String, F) 用于响应式条件类
impl<F> IntoStorable for (String, F)
where
    F: Fn() -> bool + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// (&str, F) 用于响应式条件类 - 转换为 (String, F)
impl<F> IntoStorable for (&str, F)
where
    F: Fn() -> bool + 'static,
{
    type Stored = (String, F);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

// (String, ReadSignal<bool>) 用于 Signal 条件类
impl IntoStorable for (String, ReadSignal<bool>) {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for (&str, ReadSignal<bool>) {
    type Stored = (String, ReadSignal<bool>);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

impl IntoStorable for (String, RwSignal<bool>) {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for (&str, RwSignal<bool>) {
    type Stored = (String, RwSignal<bool>);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

impl IntoStorable for (String, Signal<bool>) {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for (&str, Signal<bool>) {
    type Stored = (String, Signal<bool>);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

impl IntoStorable for (String, Memo<bool>) {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for (&str, Memo<bool>) {
    type Stored = (String, Memo<bool>);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

// --- IntoStorable 实现：Style 键值对元组 ---

// (&str, &str) 用于 Style 键值对
impl IntoStorable for (&str, &str) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1.to_string())
    }
}

impl IntoStorable for (&str, String) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0.to_string(), self.1)
    }
}

impl IntoStorable for (String, &str) {
    type Stored = (String, String);
    fn into_storable(self) -> Self::Stored {
        (self.0, self.1.to_string())
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
