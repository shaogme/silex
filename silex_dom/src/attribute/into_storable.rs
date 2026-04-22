use super::{ApplyToDom, AttributeGroup};
use crate::view::Prop;
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

impl IntoStorable for &'static str {
    type Stored = &'static str;
    fn into_storable(self) -> Self::Stored {
        self
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
                #[inline]
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
macro_rules! impl_into_storable_rx {
    (($($gen:tt)*) => $ty:ty) => {
        impl<$($gen)*> IntoStorable for $ty
        where
            Self: silex_core::traits::RxBase + silex_core::traits::IntoRx + 'static,
            <Self as silex_core::traits::IntoRx>::RxType: super::ApplyToDom + 'static,
        {
            type Stored = <Self as silex_core::traits::IntoRx>::RxType;
            #[inline(always)]
            fn into_storable(self) -> Self::Stored {
                use silex_core::traits::IntoRx;
                self.into_rx()
            }
        }
    };
}

impl_into_storable_rx!((V, M) => silex_core::Rx<V, M>);
impl_into_storable_rx!((T) => silex_core::reactivity::Signal<T>);
impl_into_storable_rx!((T) => silex_core::reactivity::ReadSignal<T>);
impl_into_storable_rx!((T) => silex_core::reactivity::RwSignal<T>);
impl_into_storable_rx!((T) => silex_core::reactivity::Constant<T>);
impl_into_storable_rx!((T) => silex_core::reactivity::Memo<T>);
impl_into_storable_rx!((S, F) => silex_core::reactivity::DerivedPayload<S, F>);
impl_into_storable_rx!((S, F, O) => silex_core::reactivity::SignalSlice<S, F, O>);

// --- 3. 静态载体与逃逸舱 ---

impl IntoStorable for super::AttrOp {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl IntoStorable for super::PendingAttribute {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 4. Tuple 实现 ---

// 统一泛型实现：(Key, Value)
impl<K, V> IntoStorable for (K, V)
where
    K: IntoStorable,
    V: IntoStorable,
    (K::Stored, V::Stored): ApplyToDom + 'static,
{
    type Stored = (K::Stored, V::Stored);

    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_storable())
    }
}

// --- IntoStorable 实现：集合类型 ---

impl<V: IntoStorable, const N: usize> IntoStorable for [V; N] {
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

impl IntoStorable for AttributeGroup {
    type Stored = AttributeGroup;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<'a, T> IntoStorable for Prop<'a, T>
where
    T: Clone + IntoStorable,
{
    type Stored = T::Stored;
    fn into_storable(self) -> Self::Stored {
        match self {
            Self::Owned(v) => v.into_storable(),
            Self::Borrowed(v) => v.clone().into_storable(),
        }
    }
}
