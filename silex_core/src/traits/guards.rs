use crate::NodeRef;
use std::cell::OnceCell;
use std::marker::PhantomData;
use std::ops::Deref;

/// 内部辅助 Trait，用于抹平所有权存储与借用目标之间的 Deref 差异。
pub trait GuardStorage<T: ?Sized> {
    fn borrow_storage(&self) -> &T;
}

impl<T: Sized> GuardStorage<T> for T {
    #[inline(always)]
    fn borrow_storage(&self) -> &T {
        self
    }
}

impl GuardStorage<str> for String {
    #[inline(always)]
    fn borrow_storage(&self) -> &str {
        self.as_str()
    }
}

/// 统一大一统的响应式守卫。
///
/// - 'a: 借用生命周期。
/// - T: 逻辑值类型（支持 ?Sized）。
/// - S: 内部存储类型（必须 Sized，默认为 ()，当需要 Owned 变体时应指定具体的类型）。
pub enum RxGuard<'a, T: ?Sized, S = ()> {
    /// 借用变体：可以是来自 Arena 的信号引用，也可以是来自 Constant 的静态引用。
    Borrowed {
        value: &'a T,
        token: Option<NodeRef>,
    },
    /// 所有权变体：持有计算结果或内联值。
    Owned(S),
}

impl<'a, T: ?Sized, S: GuardStorage<T>> Deref for RxGuard<'a, T, S> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed { value, .. } => value,
            Self::Owned(s) => s.borrow_storage(),
        }
    }
}

impl<'a, T: ?Sized, S> RxGuard<'a, T, S> {
    /// 投影借用守卫持有的引用。
    /// 仅在当前守卫为 Borrowed 时有效，否则返回 None。
    #[inline(always)]
    pub fn try_map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> Option<RxGuard<'a, U, ()>> {
        match self {
            Self::Borrowed { value, token } => Some(RxGuard::Borrowed {
                value: f(value),
                token,
            }),
            Self::Owned(_) => None,
        }
    }
}

macro_rules! impl_tuple_guard {
    ($name:ident, $($idx:tt : $meth:ident : $G:ident : $V:ident),+; $cell_idx:tt) => {
        /// 为元组设计的自适应守卫，支持分段式（零拷贝）读取。
        pub struct $name<'a, $($G, $V),+>(
            $(pub $G,)+
            pub(crate) OnceCell<($($V,)+)>,
            pub(crate) PhantomData<&'a ()>
        );

        impl<'a, $($G, $V),+> $name<'a, $($G, $V),+> {
            $(
                /// 获取该位置的分段守卫引用。
                #[inline(always)]
                pub fn $meth(&self) -> &$G {
                    &self.$idx
                }
            )+
        }

        impl<'a, $($G, $V),+> Deref for $name<'a, $($G, $V),+>
        where
            $($G: Deref<Target = $V>),+,
            $($V: Clone),+
        {
            type Target = ($($V,)+);

            fn deref(&self) -> &Self::Target {
                self.$cell_idx.get_or_init(|| {
                    ($(self.$idx.deref().clone(),)+)
                })
            }
        }
    };
}

impl_tuple_guard!(Tuple2ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1; 2);
impl_tuple_guard!(Tuple3ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2; 3);
impl_tuple_guard!(Tuple4ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2, 3: _3: G3: V3; 4);
impl_tuple_guard!(Tuple5ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2, 3: _3: G3: V3, 4: _4: G4: V4; 5);
impl_tuple_guard!(Tuple6ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2, 3: _3: G3: V3, 4: _4: G4: V4, 5: _5: G5: V5; 6);
