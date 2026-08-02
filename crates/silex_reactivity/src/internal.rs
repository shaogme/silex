//! 响应式运行时的安全内部数据结构。

pub(crate) mod value;

slotmap::new_key_type! {
    /// Generational node identifier owned by a reactive runtime scope.
    pub struct RawId;
}

impl RawId {
    /// A constant invalid/dangling node key.
    pub const DANGLING: Self = Self(slotmap::KeyData::from_ffi(u64::MAX));

    /// Returns `true` if this `RawId` is the dangling sentinel.
    #[inline]
    pub fn is_dangling(self) -> bool {
        self == Self::DANGLING
    }

    /// Returns `true` if this `RawId` represents a valid non-dangling node key.
    #[inline]
    pub fn is_valid(self) -> bool {
        self != Self::DANGLING
    }

    /// Converts an `Option<RawId>` to a `RawId`, replacing `None` with `RawId::DANGLING`.
    #[inline]
    pub fn from_option(opt: Option<Self>) -> Self {
        opt.unwrap_or(Self::DANGLING)
    }

    /// Converts this `RawId` to `Option<RawId>`, returning `None` if `is_dangling()`.
    #[inline]
    pub fn to_option(self) -> Option<Self> {
        if self.is_dangling() { None } else { Some(self) }
    }
}
