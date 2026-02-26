use super::{ApplyToDom, AttributeGroup};
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
// 所有通过 rx!(...) 或 .into_rx() 创建的统一外衣在这里直接支持（作为 'static 类型自转）

impl<F> IntoStorable for silex_core::Rx<F, silex_core::RxValueKind>
where
    Self: silex_core::traits::IntoSignal + 'static,
    <Self as silex_core::traits::RxValue>::Value: silex_core::traits::RxCloneData + Sized,
    silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>:
        ApplyToDom + 'static,
{
    type Stored = silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoSignal;
        self.into_signal()
    }
}

impl<F> IntoStorable for silex_core::Rx<F, silex_core::RxEffectKind>
where
    Self: ApplyToDom + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 3. 响应式归一化 (转换为 Signal) ---

impl<T> IntoStorable for silex_core::reactivity::Signal<T>
where
    Self: ApplyToDom + 'static,
{
    type Stored = Self;
    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        self
    }
}

macro_rules! impl_into_storable_to_signal {
    ($($ty:ident),*) => {
        $(
            impl<T> IntoStorable for silex_core::reactivity::$ty<T>
            where
                Self: silex_core::traits::IntoSignal + 'static,
                <Self as silex_core::traits::RxValue>::Value: silex_core::traits::RxCloneData + Sized,
                silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>:
                    ApplyToDom + 'static,
            {
                type Stored = silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>;

                #[inline(always)]
                fn into_storable(self) -> Self::Stored {
                    use silex_core::traits::IntoSignal;
                    self.into_signal()
                }
            }
        )*
    };
}

impl_into_storable_to_signal!(ReadSignal, RwSignal, Constant, Memo);

impl<S, F> IntoStorable for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoSignal + 'static,
    <Self as silex_core::traits::RxValue>::Value: silex_core::traits::RxCloneData + Sized,
    silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>:
        ApplyToDom + 'static,
{
    type Stored = silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoSignal;
        self.into_signal()
    }
}

impl<U, const N: usize> IntoStorable for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoSignal + 'static,
    <Self as silex_core::traits::RxValue>::Value: silex_core::traits::RxCloneData + Sized,
    silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>:
        ApplyToDom + 'static,
{
    type Stored = silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoSignal;
        self.into_signal()
    }
}

impl<S, F, O> IntoStorable for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoSignal + 'static,
    <Self as silex_core::traits::RxValue>::Value: silex_core::traits::RxCloneData + Sized,
    silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>:
        ApplyToDom + 'static,
{
    type Stored = silex_core::reactivity::Signal<<Self as silex_core::traits::RxValue>::Value>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoSignal;
        self.into_signal()
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
