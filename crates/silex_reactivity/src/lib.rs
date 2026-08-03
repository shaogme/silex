//! Explicit, single-threaded scoped reactivity runtime.
//!
//! A user owns a [`Runtime`] and starts one long-lived root with
//! [`Runtime::run`]. The returned [`RootHandle`] owns the root until explicit
//! disposal or Drop. Non-`'static` nodes are created through
//! [`RootScope::child`] and are destroyed before that child callback returns.
//!
//! The runtime deliberately has no thread-local fallback. Computations are
//! stored as `Box<dyn FnMut() + 'scope>` inside the state for their scope, and
//! handles retain a safe reference to their owning storage. User callbacks are always
//! invoked after the mutable state borrow has been released.
//!
//! Lexical handles cannot escape [`RootScope::child`]; root-owned capabilities
//! use an owner-backed weak state and become invalid after [`RootHandle`] ends.

#![deny(unreachable_pub)]

mod child;
mod completion;
mod error;
mod handle;
mod internal;
mod root;
mod runtime;
mod scope;

pub use crate::{
    child::*,
    completion::CompletionToken,
    error::{ReactiveError, ReactiveResult},
    handle::{
        AnyHandle, CallbackId, DerivedId, EffectId, Handle, MemoId, NodeKind, NodeKindTag,
        NodeRefId, SignalId, StoredId, kind,
    },
    root::*,
    runtime::{Runtime, RuntimeInput, RuntimeInputs},
};
