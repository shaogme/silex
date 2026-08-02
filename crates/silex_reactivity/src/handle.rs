//! Lifetime-bearing node capabilities.
//!
//! A handle contains a generation-checked internal key and a reference to the
//! `ScopeFrame` that owns it. The lifetime marker is covariant so Rust can
//! shorten a handle when it is borrowed, while `Scope::child`'s higher-ranked
//! callback prevents a child handle from being returned to its parent.

use crate::{internal::RawId, runtime::ScopeState, scope::ScopeFrame};
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
            pub type $alias<'scope, 'run> = Handle<'scope, 'run, kind::$kind>;
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
pub struct Handle<'scope, 'run, K: NodeKind> {
    pub(crate) frame: &'scope ScopeFrame<'run>,
    pub(crate) raw: RawId,
    marker: PhantomData<&'scope ()>,
    kind: PhantomData<fn() -> K>,
}

impl<'scope, 'run, K: NodeKind> Handle<'scope, 'run, K> {
    pub(crate) fn new(frame: &'scope ScopeFrame<'run>, raw: RawId) -> Self {
        Self {
            frame,
            raw,
            marker: PhantomData,
            kind: PhantomData,
        }
    }

    pub(crate) fn state(&self) -> Rc<RefCell<ScopeState<'run>>> {
        self.frame.state.clone()
    }

    pub(crate) const fn raw(&self) -> RawId {
        self.raw
    }

    /// Returns whether the owning scope still contains this node.
    pub fn is_alive(&self) -> bool {
        self.state()
            .try_borrow()
            .ok()
            .is_some_and(|state| state.node_kind(self.raw) == Some(K::TAG))
    }
}

impl<K: NodeKind> Copy for Handle<'_, '_, K> {}

impl<K: NodeKind> Clone for Handle<'_, '_, K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: NodeKind> PartialEq for Handle<'_, '_, K> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.frame.scope_id == other.frame.scope_id
    }
}

impl<K: NodeKind> Eq for Handle<'_, '_, K> {}

impl<K: NodeKind> Hash for Handle<'_, '_, K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.frame.scope_id.hash(state);
    }
}

impl<K: NodeKind> fmt::Debug for Handle<'_, '_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:?}", K::NAME, self.raw)
    }
}

/// A common diagnostic capability shared by all typed handles.
pub trait AnyHandle {
    fn is_alive(&self) -> bool;
}

impl<K: NodeKind> AnyHandle for Handle<'_, '_, K> {
    fn is_alive(&self) -> bool {
        Self::is_alive(self)
    }
}
