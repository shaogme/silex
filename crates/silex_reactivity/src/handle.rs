//! Lifetime-bearing node capabilities.
//!
//! A handle contains a generation-checked internal key and a reference to the
//! storage that owns it. The scope lifetime is invariant so handles from
//! different lexical scopes cannot be combined by lifetime shortening.

use crate::{RuntimeInput, internal::RawId, runtime::ScopeState, scope::ScopeStorage};
use std::{
    cell::RefCell,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    rc::Rc,
};

mod sealed {
    pub trait Sealed {}
}

/// Marker for the kind of node represented by a [`Handle`].
pub trait NodeKind: sealed::Sealed + 'static {
    const NAME: &str;
    const TAG: NodeKindTag;
}

#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NodeKindTag {
    Signal,
    Memo,
    Derived,
    Effect,
    Stored,
    Callback,
    NodeRef,
}

/// Zero-sized node-kind markers.
pub mod kind {
    pub struct Signal;
    pub struct Memo;
    pub struct Derived;
    pub struct Effect;
    pub struct Stored;
    pub struct Callback;
    pub struct NodeRef;
}

macro_rules! define_kinds {
    ($( $kind:ident, $alias:ident, $name:literal, $tag:ident; )*) => {
        $(
            impl sealed::Sealed for kind::$kind {}
            impl NodeKind for kind::$kind {
                const NAME: &str = $name;
                const TAG: NodeKindTag = NodeKindTag::$tag;
            }
            pub type $alias<'scope> = Handle<'scope, kind::$kind>;
        )*
    };
}

define_kinds! {
    Signal, SignalId, "signal", Signal;
    Memo, MemoId, "memo", Memo;
    Derived, DerivedId, "derived", Derived;
    Effect, EffectId, "effect", Effect;
    Stored, StoredId, "stored value", Stored;
    Callback, CallbackId, "callback", Callback;
    NodeRef, NodeRefId, "node ref", NodeRef;
}

/// A node capability tied to the lexical scope that created it.
pub struct Handle<'scope, K: NodeKind> {
    pub(crate) storage: &'scope ScopeStorage,
    pub(crate) raw: RawId,
    marker: PhantomData<fn(&'scope ()) -> &'scope ()>,
    kind: PhantomData<fn() -> K>,
}

impl<'scope, K: NodeKind> Handle<'scope, K> {
    pub(crate) fn new(storage: &'scope ScopeStorage, raw: RawId) -> Self {
        Self {
            storage,
            raw,
            marker: PhantomData,
            kind: PhantomData,
        }
    }

    pub(crate) fn state(&self) -> Rc<RefCell<ScopeState<'scope>>> {
        // SAFETY: the handle lifetime is tied to the lexical Scope capability
        // that created it, and storage is disposed before that capability can
        // leave its higher-ranked callback.
        unsafe { self.storage.typed_state() }
    }

    pub(crate) const fn raw(&self) -> RawId {
        self.raw
    }

    /// Return opaque scheduler-family provenance for this node.
    #[doc(hidden)]
    pub(crate) fn runtime_input(&self) -> RuntimeInput {
        RuntimeInput::from_scheduler(self.storage.scheduler())
    }
}

impl<K: NodeKind> Copy for Handle<'_, K> {}

impl<K: NodeKind> Clone for Handle<'_, K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: NodeKind> PartialEq for Handle<'_, K> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.storage.scope_id == other.storage.scope_id
    }
}

impl<K: NodeKind> Eq for Handle<'_, K> {}

impl<K: NodeKind> Hash for Handle<'_, K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.storage.scope_id.hash(state);
    }
}

impl<K: NodeKind> fmt::Debug for Handle<'_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:?}", K::NAME, self.raw)
    }
}
