//! 分相类型（phantom-typed）的节点句柄。
//!
//! 从前整个 crate 只对外交出一个通用节点句柄，而 signal / memo / derived / effect /
//! scope / stored value / closure / callback / node-ref **九种语义完全不同的东西
//! 共用它**。后果是所有种类错误都退化成运行时的一个 `None`：
//!
//! ```ignore
//! let sv = store_value(42i32);
//! let v = try_get_signal::<i32>(sv);   // 编译通过，运行时静默返回 None
//! ```
//!
//! 于是 crate 不得不提供六个近乎一模一样的运行时探测函数（`is_signal_valid` /
//! `is_stored_value_valid` / …）来补救（审计报告 §3.1）。
//!
//! 现在句柄带上了一个零大小的种类标记：
//!
//! ```ignore
//! let sv: StoredId = store::create(42i32);
//! let v = signal::try_get::<i32>(sv);   // ← 编译错误
//! ```
//!
//! - **运行时表示不变**：`Handle<K>` 是 `#[repr(transparent)]` 包着的 8 字节
//!   `{index, generation}`，零开销；
//! - 六个 `is_*_valid` 收敛成一个 [`Handle::is_alive`]；
//! - 需要跨种类传递时（下游框架的类型擦除分发）用 [`RawId`] 显式擦除，
//!   这是唯一的逃生出口，也因此是唯一需要人工审查的地方。

use crate::runtime::with_rt;
use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

/// 类型擦除的句柄：8 字节的 `{槽位, 代数}`。
///
/// 这是所有 [`Handle<K>`] 的运行时表示，也是跨种类传递时的显式逃生出口
/// （下游框架用运行时 tag 自己分发时需要它）。它自己**不带种类信息**，
/// 因此用它调用的每一个 API 都退回到“类型错了就返回
/// [`WrongKind`](crate::ReactiveError::WrongKind)”的运行时检查。
pub use crate::internal::arena::RawId;

mod sealed {
    pub trait Sealed {}
}

/// 一种节点的分相标记。
///
/// 实现者是 [`kind`] 里那几个零大小类型，本 trait 对外封闭。
pub trait NodeKind: sealed::Sealed + 'static {
    /// 出现在诊断信息里的名字。
    const NAME: &'static str;

    /// 这个擦除句柄是否仍指向一个活着的、本种类的节点。
    #[doc(hidden)]
    fn alive(raw: RawId) -> bool;
}

/// 节点种类的标记类型。它们只出现在类型位置上，没有值。
pub mod kind {
    /// 图的根：存一个值，每次写入都通知下游。
    pub struct Signal;
    /// 惰性派生值，带相等性门控。
    pub struct Memo;
    /// 惰性派生值，不做门控。
    pub struct Derived;
    /// 副作用节点。
    pub struct Effect;
    /// 纯所有权容器。
    pub struct Scope;
    /// 非响应式的保管值。
    pub struct Stored;
    /// 类型擦除的回调。
    pub struct Callback;
    /// “稍后填充”的宿主元素引用。
    pub struct NodeRef;
}

macro_rules! define_kinds {
    ($( $kind:ident, $alias:ident, $name:literal, $alive:expr; )*) => {
        $(
            impl sealed::Sealed for kind::$kind {}
            impl NodeKind for kind::$kind {
                const NAME: &'static str = $name;
                #[inline]
                fn alive(raw: RawId) -> bool {
                    let alive: fn(&crate::Runtime, RawId) -> bool = $alive;
                    with_rt(|rt| alive(rt, raw)).unwrap_or(false)
                }
            }
            #[doc = concat!("指向一个 ", $name, " 的句柄。")]
            pub type $alias = Handle<kind::$kind>;
        )*
    };
}

// signal / memo / derived 三者在运行时是同一种东西（都有 `signal` 载荷、都可读），
// 区别只在门控策略，因此存活判定共用一条。effect 没有值，单独一条。
define_kinds! {
    Signal,   SignalId,   "signal",       |rt, id| rt.node_has_value(id);
    Memo,     MemoId,     "memo",         |rt, id| rt.node_has_value(id);
    Derived,  DerivedId,  "derived",      |rt, id| rt.node_has_value(id);
    Effect,   EffectId,   "effect",       |rt, id| rt.node_is_effect(id);
    Scope,    ScopeId,    "scope",        |rt, id| rt.node_exists(id);
    Stored,   StoredId,   "stored value", |rt, id| rt.node_has_payload(id);
    Callback, CallbackId, "callback",     |rt, id| rt.node_has_payload(id);
    NodeRef,  NodeRefId,  "node ref",     |rt, id| rt.node_has_payload(id);
}

/// 一个带种类标记的节点句柄。
///
/// `Copy`、8 字节、可以随便传递 —— 但**它不保证节点还活着**，用
/// [`Handle::is_alive`] 查。
#[repr(transparent)]
pub struct Handle<K: NodeKind> {
    raw: RawId,
    /// `fn() -> K` 而不是 `K`：让 `Handle<K>` 对 `K` 保持协变，且不会因为标记
    /// 类型不是 `Send`/`Sync` 而把句柄本身也拖下水。
    _kind: PhantomData<fn() -> K>,
}

impl<K: NodeKind> Handle<K> {
    /// 一个永远不指向任何节点的句柄，供下游框架的 `Default` 实现占位用。
    pub const DANGLING: Self = Self::from_raw(RawId::DANGLING);

    #[inline(always)]
    pub(crate) const fn from_raw(raw: RawId) -> Self {
        Self {
            raw,
            _kind: PhantomData,
        }
    }

    /// 擦除种类，得到异构图使用的句柄。
    #[inline(always)]
    pub const fn raw(self) -> RawId {
        self.raw
    }

    /// 该句柄是否仍指向一个活着的、本种类的节点。
    ///
    /// 这一个方法取代了从前的 `is_signal_valid` / `is_stored_value_valid` /
    /// `is_closure_valid` / `is_op_valid` / `is_callback_valid` /
    /// `is_node_ref_valid` 六个函数（审计报告 §3.1）。
    #[inline]
    pub fn is_alive(self) -> bool {
        K::alive(self.raw)
    }

    /// 把一个擦除句柄**断言**成本种类。
    ///
    /// # Safety
    ///
    /// 这不是 `unsafe fn`（用错种类不会造成内存不安全，只会让后续调用返回
    /// [`WrongKind`](crate::ReactiveError::WrongKind) 或
    /// [`TypeMismatch`](crate::ReactiveError::TypeMismatch)），但它**绕过了本模块
    /// 存在的全部意义**。只有在种类信息已经由调用方用别的方式维持住时才用它 ——
    /// 典型场景是下游框架自己带了一个运行时 tag 的枚举。
    #[inline(always)]
    pub const fn from_raw_unchecked(raw: RawId) -> Self {
        Self::from_raw(raw)
    }
}

impl<K: NodeKind> Clone for Handle<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: NodeKind> Copy for Handle<K> {}

impl<K: NodeKind> PartialEq for Handle<K> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<K: NodeKind> Eq for Handle<K> {}

impl<K: NodeKind> Hash for Handle<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<K: NodeKind> fmt::Debug for Handle<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:?}", K::NAME, self.raw)
    }
}

/// 任何句柄 —— 用于对种类无所谓的操作（调试标签、原始指针逃生出口）。
pub trait AnyHandle: Copy + sealed::Sealed {
    /// 擦除种类。
    fn into_raw(self) -> RawId;
}

impl<K: NodeKind> sealed::Sealed for Handle<K> {}
impl<K: NodeKind> AnyHandle for Handle<K> {
    #[inline(always)]
    fn into_raw(self) -> RawId {
        self.raw
    }
}

impl sealed::Sealed for RawId {}
impl AnyHandle for RawId {
    #[inline(always)]
    fn into_raw(self) -> RawId {
        self
    }
}

/// 可以被读取并建立依赖的句柄：[`SignalId`] / [`MemoId`] / [`DerivedId`]。
///
/// [`RawId`] 也实现了它 —— 那是**类型擦除的逃生出口**，把种类检查退回到
/// 运行时。下游框架用运行时 tag 自己分发时需要它，其余场合请用带种类的句柄。
pub trait Readable: AnyHandle {}

impl Readable for SignalId {}
impl Readable for MemoId {}
impl Readable for DerivedId {}
impl Readable for RawId {}

#[cfg(test)]
mod tests {
    use super::*;

    /// 句柄必须是零开销的：和原始句柄一样大、一样对齐。
    #[test]
    fn a_typed_handle_costs_nothing() {
        assert_eq!(
            std::mem::size_of::<SignalId>(),
            std::mem::size_of::<RawId>()
        );
        assert_eq!(
            std::mem::align_of::<SignalId>(),
            std::mem::align_of::<RawId>()
        );
        assert_eq!(std::mem::size_of::<SignalId>(), 8);
    }

    #[test]
    fn every_handle_has_the_raw_layout() {
        macro_rules! assert_layout {
            ($handle:ty) => {
                assert_eq!(std::mem::size_of::<$handle>(), std::mem::size_of::<RawId>());
                assert_eq!(
                    std::mem::align_of::<$handle>(),
                    std::mem::align_of::<RawId>()
                );
            };
        }

        assert_layout!(SignalId);
        assert_layout!(MemoId);
        assert_layout!(DerivedId);
        assert_layout!(EffectId);
        assert_layout!(ScopeId);
        assert_layout!(StoredId);
        assert_layout!(CallbackId);
        assert_layout!(NodeRefId);
    }

    #[test]
    fn raw_and_typed_handles_hash_identically() {
        use std::collections::hash_map::DefaultHasher;

        let raw = RawId::new(7, 3);
        let typed = SignalId::from_raw(raw);
        let mut raw_hasher = DefaultHasher::new();
        let mut typed_hasher = DefaultHasher::new();
        raw.hash(&mut raw_hasher);
        typed.hash(&mut typed_hasher);
        assert_eq!(raw_hasher.finish(), typed_hasher.finish());
    }

    /// 空句柄对每一种查询都是“不存在”，而且不触发任何分配。
    #[test]
    fn the_dangling_handle_is_never_alive() {
        assert!(!SignalId::DANGLING.is_alive());
        assert!(!StoredId::DANGLING.is_alive());
        assert!(!ScopeId::DANGLING.is_alive());
    }
}
