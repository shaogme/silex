use std::borrow::Cow;

use super::{ApplyToDom, Attr, AttrOp, AttributeGroup};
use crate::view::Prop;
use silex_core::{Rx, RxValueKind};
// --- IntoStorable: 允许非 'static 类型转换为可存储类型 ---

/// 将值转换为可存储的类型。
/// 对于引用类型（如 &str, &String），转换为 owned 类型（String）。
/// 对于已经是 'static 的类型，直接返回自身。
pub trait IntoStorable<'scope, 'run> {
    /// 转换后的可存储类型必须受当前 view scope 约束。
    type Stored: ApplyToDom<'scope, 'run> + 'scope;

    /// 将自身转换为可存储类型
    fn into_storable(self) -> Self::Stored;
}

// --- 1. 基础类型 ---

impl<'scope, 'run> IntoStorable<'scope, 'run> for &'static str {
    type Stored = &'static str;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'run> IntoStorable<'scope, 'run> for &String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self.clone()
    }
}

impl<'scope, 'run> IntoStorable<'scope, 'run> for String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'run> IntoStorable<'scope, 'run> for Cow<'static, str> {
    type Stored = Cow<'static, str>;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'run> IntoStorable<'scope, 'run> for bool {
    type Stored = bool;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

macro_rules! impl_into_storable_primitive {
    ($($t:ty),*) => {
        $(
            impl<'scope, 'run> IntoStorable<'scope, 'run> for $t {
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

// --- 2. Rx 支持 ---
impl<'scope, 'run, T> IntoStorable<'scope, 'run> for Rx<'scope, 'run, T, RxValueKind>
where
    T: super::ReactiveApply<'scope, 'run> + Clone + 'scope,
{
    type Stored = Self;

    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'run, T> IntoStorable<'scope, 'run> for silex_core::reactivity::Signal<'scope, 'run, T>
where
    T: super::ReactiveApply<'scope, 'run> + Clone + 'scope,
{
    type Stored = Rx<'scope, 'run, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, 'run, T> IntoStorable<'scope, 'run>
    for silex_core::reactivity::ReadSignal<'scope, 'run, T>
where
    T: super::ReactiveApply<'scope, 'run> + Clone + 'scope,
{
    type Stored = Rx<'scope, 'run, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, 'run, T> IntoStorable<'scope, 'run>
    for silex_core::reactivity::RwSignal<'scope, 'run, T>
where
    T: super::ReactiveApply<'scope, 'run> + Clone + 'scope,
{
    type Stored = Rx<'scope, 'run, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, 'run, T> IntoStorable<'scope, 'run> for silex_core::reactivity::Memo<'scope, 'run, T>
where
    T: super::ReactiveApply<'scope, 'run> + Clone + 'scope,
{
    type Stored = Rx<'scope, 'run, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, 'run, T> IntoStorable<'scope, 'run>
    for silex_core::reactivity::StoredValue<'scope, 'run, T>
where
    T: super::ReactiveApply<'scope, 'run> + Clone + 'scope,
{
    type Stored = Rx<'scope, 'run, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

// --- 3. 静态载体与逃逸舱 ---

impl<'scope, 'run> IntoStorable<'scope, 'run> for Attr {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'run> IntoStorable<'scope, 'run> for AttrOp<'scope, 'run> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 4. Tuple 实现 ---

// 统一泛型实现：(Key, Value)
impl<'scope, 'run, K, V> IntoStorable<'scope, 'run> for (K, V)
where
    K: IntoStorable<'scope, 'run>,
    V: IntoStorable<'scope, 'run>,
    (K::Stored, V::Stored): ApplyToDom<'scope, 'run> + 'scope,
{
    type Stored = (K::Stored, V::Stored);

    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_storable())
    }
}

// --- IntoStorable 实现：集合类型 ---

impl<'scope, 'run, V: IntoStorable<'scope, 'run>, const N: usize> IntoStorable<'scope, 'run>
    for [V; N]
where
    V::Stored: 'scope,
{
    type Stored = [V::Stored; N];
    fn into_storable(self) -> Self::Stored {
        self.map(|v| v.into_storable())
    }
}

impl<'scope, 'run, V: IntoStorable<'scope, 'run>> IntoStorable<'scope, 'run> for Option<V>
where
    V::Stored: 'scope,
{
    type Stored = Option<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.map(|v| v.into_storable())
    }
}

impl<'scope, 'run, V: IntoStorable<'scope, 'run>> IntoStorable<'scope, 'run> for Vec<V>
where
    V::Stored: 'scope,
{
    type Stored = Vec<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.into_iter().map(|v| v.into_storable()).collect()
    }
}

// --- IntoStorable 实现：AttributeGroup ---

impl<'scope, 'run> IntoStorable<'scope, 'run> for AttributeGroup<'scope, 'run> {
    type Stored = AttributeGroup<'scope, 'run>;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'run, 'a, T> IntoStorable<'scope, 'run> for Prop<'a, T>
where
    'a: 'scope,
    T: Clone + IntoStorable<'scope, 'run>,
    T::Stored: 'scope,
{
    type Stored = T::Stored;
    fn into_storable(self) -> Self::Stored {
        match self {
            Self::Owned(v) => v.into_storable(),
            Self::Borrowed(v) => v.clone().into_storable(),
        }
    }
}
