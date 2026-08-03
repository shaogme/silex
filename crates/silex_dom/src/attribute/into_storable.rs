use std::borrow::Cow;

use super::{ApplyToDom, Attr, AttrOp, AttributeGroup};
use crate::view::Prop;
use silex_core::{Rx, RxValueKind};
// --- IntoStorable: 允许非 'static 类型转换为可存储类型 ---

/// 将值转换为可存储的类型。
/// 对于引用类型（如 &str, &String），转换为 owned 类型（String）。
/// 对于已经是 'static 的类型，直接返回自身。
pub trait IntoStorable<'scope> {
    /// 转换后的可存储类型必须受当前 view scope 约束。
    type Stored: ApplyToDom<'scope> + 'scope;

    /// 将自身转换为可存储类型
    fn into_storable(self) -> Self::Stored;
}

// --- 1. 基础类型 ---

impl<'scope> IntoStorable<'scope> for &'static str {
    type Stored = &'static str;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope> IntoStorable<'scope> for &String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self.clone()
    }
}

impl<'scope> IntoStorable<'scope> for String {
    type Stored = String;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope> IntoStorable<'scope> for Cow<'static, str> {
    type Stored = Cow<'static, str>;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope> IntoStorable<'scope> for bool {
    type Stored = bool;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

macro_rules! impl_into_storable_primitive {
    ($($t:ty),*) => {
        $(
            impl<'scope> IntoStorable<'scope> for $t {
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
impl<'scope, T> IntoStorable<'scope> for Rx<'scope, T, RxValueKind>
where
    T: super::ReactiveApply<'scope> + Clone + 'scope,
{
    type Stored = Self;

    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::Signal<'scope, T>
where
    T: super::ReactiveApply<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::ReadSignal<'scope, T>
where
    T: super::ReactiveApply<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::RwSignal<'scope, T>
where
    T: super::ReactiveApply<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::Memo<'scope, T>
where
    T: super::ReactiveApply<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

impl<'scope, T> IntoStorable<'scope> for silex_core::reactivity::StoredValue<'scope, T>
where
    T: super::ReactiveApply<'scope> + Clone + 'scope,
{
    type Stored = Rx<'scope, T, RxValueKind>;

    fn into_storable(self) -> Self::Stored {
        self.into_rx()
    }
}

// --- 3. 静态载体与逃逸舱 ---

impl<'scope> IntoStorable<'scope> for Attr {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope> IntoStorable<'scope> for AttrOp<'scope> {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 4. Tuple 实现 ---

// 统一泛型实现：(Key, Value)
impl<'scope, K, V> IntoStorable<'scope> for (K, V)
where
    K: IntoStorable<'scope>,
    V: IntoStorable<'scope>,
    (K::Stored, V::Stored): ApplyToDom<'scope> + 'scope,
{
    type Stored = (K::Stored, V::Stored);

    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_storable())
    }
}

// --- IntoStorable 实现：集合类型 ---

impl<'scope, V: IntoStorable<'scope>, const N: usize> IntoStorable<'scope> for [V; N]
where
    V::Stored: 'scope,
{
    type Stored = [V::Stored; N];
    fn into_storable(self) -> Self::Stored {
        self.map(|v| v.into_storable())
    }
}

impl<'scope, V: IntoStorable<'scope>> IntoStorable<'scope> for Option<V>
where
    V::Stored: 'scope,
{
    type Stored = Option<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.map(|v| v.into_storable())
    }
}

impl<'scope, V: IntoStorable<'scope>> IntoStorable<'scope> for Vec<V>
where
    V::Stored: 'scope,
{
    type Stored = Vec<V::Stored>;
    fn into_storable(self) -> Self::Stored {
        self.into_iter().map(|v| v.into_storable()).collect()
    }
}

// --- IntoStorable 实现：AttributeGroup ---

impl<'scope> IntoStorable<'scope> for AttributeGroup<'scope> {
    type Stored = AttributeGroup<'scope>;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'scope, 'a, T> IntoStorable<'scope> for Prop<'a, T>
where
    'a: 'scope,
    T: Clone + IntoStorable<'scope>,
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
