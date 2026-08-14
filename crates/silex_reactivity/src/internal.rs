//! 响应式运行时的安全内部数据结构。

slotmap::new_key_type! {
    /// Generational node identifier owned by a reactive runtime scope.
    pub(crate) struct RawId;
}

impl RawId {
    /// A constant invalid/dangling node key.
    pub(crate) const DANGLING: Self = Self(slotmap::KeyData::from_ffi(u64::MAX));

    /// Returns `true` if this `RawId` is the dangling sentinel.
    #[inline]
    pub(crate) fn is_dangling(self) -> bool {
        self == Self::DANGLING
    }

    /// Returns `true` if this `RawId` represents a valid non-dangling node key.
    #[inline]
    pub(crate) fn is_valid(self) -> bool {
        self != Self::DANGLING
    }

    /// Converts an `Option<RawId>` to a `RawId`, replacing `None` with `RawId::DANGLING`.
    #[inline]
    pub(crate) fn from_option(opt: Option<Self>) -> Self {
        opt.unwrap_or(Self::DANGLING)
    }
}
