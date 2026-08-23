//! Lifetime-bearing node capabilities.
//!
//! A handle contains a generation-checked internal key and a reference to the
//! storage that owns it. The scope lifetime is invariant so handles from
//! different lexical scopes cannot be combined by lifetime shortening.

use crate::{internal::NodeId, owner::ScopeStorage, runtime::ScopeState};
use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

mod sealed {
    pub(crate) trait Sealed {}
}

/// Marker for the kind of node represented by a [`Handle`].
pub(crate) trait NodeKind: sealed::Sealed + 'static {
    const NAME: &str;
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum NodeKindTag {
    Signal,
    Computed,
    Effect,
    Stored,
    Callback,
    NodeRef,
}

/// Zero-sized node-kind markers.
pub(crate) mod kind {
    pub(crate) struct Signal;
    pub(crate) struct Computed;
    pub(crate) struct Effect;
    pub(crate) struct Stored;
    pub(crate) struct Callback;
    pub(crate) struct NodeRef;
}

macro_rules! define_kinds {
    ($( $kind:ident, $alias:ident, $name:literal; )*) => {
        $(
            impl sealed::Sealed for kind::$kind {}
            impl NodeKind for kind::$kind {
                const NAME: &str = $name;
            }
            pub(crate) type $alias<'scope> = Handle<'scope, kind::$kind>;
        )*
    };
}

define_kinds! {
    Signal, SignalId, "signal";
    Computed, ComputedId, "computed";
    Effect, EffectId, "effect";
    Stored, StoredId, "stored value";
    Callback, CallbackId, "callback";
    NodeRef, NodeRefId, "node ref";
}

/// A node capability tied to the lexical scope that created it.
pub(crate) struct Handle<'scope, K: NodeKind> {
    pub(crate) storage: &'scope ScopeStorage,
    pub(crate) raw: NodeId,
    marker: PhantomData<fn(&'scope ()) -> &'scope ()>,
    kind: PhantomData<fn() -> K>,
}

impl<'scope, K: NodeKind> Handle<'scope, K> {
    pub(crate) fn new(storage: &'scope ScopeStorage, raw: NodeId) -> Self {
        Self {
            storage,
            raw,
            marker: PhantomData,
            kind: PhantomData,
        }
    }

    pub(crate) fn state(&self) -> ScopeState<'scope> {
        self.storage.owner_token().state()
    }

    pub(crate) const fn raw(&self) -> NodeId {
        self.raw
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
        self.raw == other.raw && self.storage.owner_id == other.storage.owner_id
    }
}

impl<K: NodeKind> Eq for Handle<'_, K> {}

impl<K: NodeKind> Hash for Handle<'_, K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.storage.owner_id.hash(state);
    }
}

impl<K: NodeKind> fmt::Debug for Handle<'_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:?}", K::NAME, self.raw)
    }
}
