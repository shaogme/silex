//! Explicit, single-threaded scoped reactivity runtime.
//!
//! A user owns a [`Runtime`] and enters it with [`Runtime::run`]. Every node is
//! created through the [`Scope`] passed to that run and carries a lifetime tied
//! to the scope. Child scopes are lexical: their nodes are destroyed before the
//! child callback returns.
//!
//! The runtime deliberately has no thread-local fallback. Computations are
//! stored as `Box<dyn FnMut() + 'scope>` inside the state for their scope, and
//! handles retain a safe reference to their owning frame. User callbacks are always
//! invoked after the mutable state borrow has been released.
//!
//! Handles cannot escape the `Runtime::run` callback; the compile-fail case is
//! covered by `tests/ui/fail_root_handle_escape.rs`.

#![deny(unreachable_pub)]

pub mod completion;
mod error;
mod handle;
mod internal;
pub mod node;
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
    runtime::Runtime,
    scope::Scope,
};
