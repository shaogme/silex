use super::{ApplyToDom, AttributeGroup};
use silex_core::traits::{IntoRx, IntoSignal};

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

// --- 2. Rx 支持 ---
// 所有通过 rx!(...) 或 .into_rx() 创建的统一外衣在这里被支持

impl<F, M> IntoStorable for silex_core::Rx<F, M>
where
    Self: ApplyToDom + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// 代理宏：让内置响应式类型在作为属性时自动套一层 Rx 归一化
macro_rules! impl_into_storable_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> IntoStorable for silex_core::reactivity::$ty<T>
            where
                T: crate::attribute::ReactiveApply + Clone + 'static,
                Self: silex_core::traits::IntoSignal<Value = T> + Clone + 'static,
            {
                type Stored = silex_core::Rx<silex_core::reactivity::Signal<T>, silex_core::RxValueKind>;
                fn into_storable(self) -> Self::Stored {
                    silex_core::Rx(self.into_signal(), std::marker::PhantomData)
                }
            }
        )*
    };
}

impl_into_storable_signal!(Signal, ReadSignal, RwSignal);

impl<T> IntoStorable for silex_core::reactivity::Constant<T>
where
    T: crate::attribute::ReactiveApply + Clone + 'static,
{
    type Stored = silex_core::Rx<silex_core::reactivity::Signal<T>, silex_core::RxValueKind>;
    fn into_storable(self) -> Self::Stored {
        silex_core::Rx(self.into_signal(), std::marker::PhantomData)
    }
}

impl<T> IntoStorable for silex_core::reactivity::Memo<T>
where
    T: crate::attribute::ReactiveApply + Clone + PartialEq + 'static,
{
    type Stored = silex_core::Rx<silex_core::reactivity::Signal<T>, silex_core::RxValueKind>;
    fn into_storable(self) -> Self::Stored {
        silex_core::Rx(self.into_signal(), std::marker::PhantomData)
    }
}

impl<S, F> IntoStorable for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoSignal + silex_core::traits::RxValue + Clone + 'static,
    <Self as silex_core::traits::RxValue>::Value: crate::attribute::ReactiveApply + Clone + 'static,
{
    type Stored = silex_core::Rx<
        silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>,
        silex_core::RxValueKind,
    >;
    fn into_storable(self) -> Self::Stored {
        silex_core::Rx(self.into_signal(), std::marker::PhantomData)
    }
}

impl<U, const N: usize> IntoStorable for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoSignal + silex_core::traits::RxValue<Value = U> + Clone + 'static,
    U: crate::attribute::ReactiveApply + Clone + 'static,
{
    type Stored = silex_core::Rx<silex_core::reactivity::Signal<U>, silex_core::RxValueKind>;
    fn into_storable(self) -> Self::Stored {
        silex_core::Rx(self.into_signal(), std::marker::PhantomData)
    }
}

impl<S, F, O> IntoStorable for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoSignal + silex_core::traits::RxValue<Value = O> + Clone + 'static,
    O: crate::attribute::ReactiveApply + Clone + 'static,
{
    type Stored = silex_core::Rx<silex_core::reactivity::Signal<O>, silex_core::RxValueKind>;
    fn into_storable(self) -> Self::Stored {
        silex_core::Rx(self.into_signal(), std::marker::PhantomData)
    }
}

// --- 4. Tuple 实现 ---

// 统一泛型实现：(Key, Value)
// 适用于 (String, String) [Style], (String, bool) [Class], (String, Signal) 等所有情况
// 通过 ApplyToDom 的 generic impl 和 ReactiveApply::apply_pair 分发逻辑
impl<K, V> IntoStorable for (K, V)
where
    K: IntoStorable<Stored = String>,
    V: IntoRx,
    V::RxType: 'static,
    (String, V::RxType): ApplyToDom,
{
    type Stored = (String, V::RxType);

    fn into_storable(self) -> Self::Stored {
        (self.0.into_storable(), self.1.into_rx())
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
