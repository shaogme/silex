//! Explicit, single-threaded scoped reactivity runtime.
//!
//! A user owns a [`Runtime`] and starts one long-lived root with
//! [`Runtime::run`]. The returned [`RootHandle`] owns the root until explicit
//! disposal or Drop. Non-`'static` nodes are created through
//! [`RootScope::scope`] and are destroyed before that child callback returns.
//!
//! The runtime deliberately has no thread-local fallback. Computations are
//! stored as `Box<dyn FnMut() + 'scope>` inside the state for their scope, and
//! handles retain a safe reference to their owning frame. User callbacks are always
//! invoked after the mutable state borrow has been released.
//!
//! Lexical handles cannot escape [`RootScope::scope`]; root-owned capabilities
//! use an owner-backed weak state and become invalid after [`RootHandle`] ends.

#![deny(unreachable_pub)]

pub mod completion;
mod error;
mod handle;
mod internal;
pub mod node;
mod root;
mod runtime;
pub mod scope;

pub use crate::{
    completion::CompletionToken,
    error::{ReactiveError, ReactiveResult},
    handle::{
        AnyHandle, CallbackId, DerivedId, EffectId, Handle, MemoId, NodeKind, NodeKindTag,
        NodeRefId, SignalId, StoredId, kind,
    },
    internal::{RawId, value::AnyValue},
    node::{
        Callback, Derived, Effect, Memo, NodeRef, ReadSignal, RwSignal, Signal, StoredValue,
        WriteSignal, notify, track, track_batch,
    },
    root::{
        CleanupError, RootCallback, RootDerived, RootEffect, RootHandle, RootMemo, RootNodeRef,
        RootReadSignal, RootScope, RootSignal, RootStoredValue, RootWriteSignal,
    },
    runtime::Runtime,
    scope::Scope,
};
