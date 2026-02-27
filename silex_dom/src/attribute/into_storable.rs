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
// 所有通过 rx!(...) 或 .into_rx() 创建的统一外衣在这里直接支持（作为 'static 类型自转）

impl<V> IntoStorable for silex_core::Rx<V, silex_core::RxValueKind>
where
    V: silex_core::traits::RxCloneData + Sized + 'static,
    Self: ApplyToDom + 'static,
{
    type Stored = Self;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        self
    }
}

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

impl IntoStorable
    for silex_core::Rx<std::rc::Rc<dyn Fn(&web_sys::Element)>, silex_core::RxEffectKind>
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

// --- 3. 响应式归一化 (转换为 Rx) ---

impl<T> IntoStorable for silex_core::reactivity::Signal<T>
where
    T: silex_core::traits::RxCloneData,
    silex_core::Rx<T, silex_core::RxValueKind>: ApplyToDom + 'static,
{
    type Stored = silex_core::Rx<T, silex_core::RxValueKind>;
    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoRx;
        self.into_rx()
    }
}

macro_rules! impl_into_storable_to_rx {
    ($($ty:ident),*) => {
        $(
            impl<T> IntoStorable for silex_core::reactivity::$ty<T>
            where
                T: silex_core::traits::RxCloneData + Sized + 'static,
                Self: silex_core::traits::IntoRx<RxType = silex_core::Rx<T, silex_core::RxValueKind>> + 'static,
                silex_core::Rx<T, silex_core::RxValueKind>: ApplyToDom + 'static,
            {
                type Stored = silex_core::Rx<T, silex_core::RxValueKind>;

                #[inline(always)]
                fn into_storable(self) -> Self::Stored {
                    use silex_core::traits::IntoRx;
                    self.into_rx()
                }
            }
        )*
    };
}

impl_into_storable_to_rx!(ReadSignal, RwSignal, Constant, Memo);

impl<S, F> IntoStorable for silex_core::reactivity::DerivedPayload<S, F>
where
    Self: silex_core::traits::IntoRx<
            RxType = silex_core::Rx<
                <Self as silex_core::traits::RxValue>::Value,
                silex_core::RxValueKind,
            >,
        > + 'static,
    <Self as silex_core::traits::RxValue>::Value: silex_core::traits::RxCloneData + Sized,
    silex_core::Rx<<Self as silex_core::traits::RxValue>::Value, silex_core::RxValueKind>:
        ApplyToDom + 'static,
{
    type Stored =
        silex_core::Rx<<Self as silex_core::traits::RxValue>::Value, silex_core::RxValueKind>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoRx;
        self.into_rx()
    }
}

impl<U, const N: usize> IntoStorable for silex_core::reactivity::OpPayload<U, N>
where
    Self: silex_core::traits::IntoRx<RxType = silex_core::Rx<U, silex_core::RxValueKind>> + 'static,
    U: silex_core::traits::RxCloneData + Sized + 'static,
    silex_core::Rx<U, silex_core::RxValueKind>: ApplyToDom + 'static,
{
    type Stored = silex_core::Rx<U, silex_core::RxValueKind>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoRx;
        self.into_rx()
    }
}

impl<S, F, O> IntoStorable for silex_core::reactivity::SignalSlice<S, F, O>
where
    Self: silex_core::traits::IntoRx<RxType = silex_core::Rx<O, silex_core::RxValueKind>> + 'static,
    O: silex_core::traits::RxCloneData + Sized + 'static,
    silex_core::Rx<O, silex_core::RxValueKind>: ApplyToDom + 'static,
{
    type Stored = silex_core::Rx<O, silex_core::RxValueKind>;

    #[inline(always)]
    fn into_storable(self) -> Self::Stored {
        use silex_core::traits::IntoRx;
        self.into_rx()
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

// --- Recursive Attribute Group Support ---

impl IntoStorable for super::AttrNil {
    type Stored = super::AttrNil;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

impl<H, T> IntoStorable for super::AttrCons<H, T>
where
    H: IntoStorable,
    T: IntoStorable,
    H::Stored: ApplyToDom + 'static,
    T::Stored: super::AttrFlatten + 'static,
{
    type Stored = super::AttrCons<H::Stored, T::Stored>;
    fn into_storable(self) -> Self::Stored {
        super::AttrCons(self.0.into_storable(), self.1.into_storable())
    }
}

// --- IntoStorable 实现：AttributeGroup ---

impl<T: IntoStorable> IntoStorable for AttributeGroup<T>
where
    T::Stored: super::AttrFlatten + 'static,
{
    type Stored = AttributeGroup<T::Stored>;
    fn into_storable(self) -> Self::Stored {
        AttributeGroup(self.0.into_storable())
    }
}
