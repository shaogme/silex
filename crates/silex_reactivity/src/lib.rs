//! Explicit, single-threaded scoped reactivity runtime.
//!
//! A user owns a [`Runtime`] and starts one long-lived root with
//! [`Runtime::run`]. The returned [`RootHandle`] owns the root until explicit
//! disposal or Drop. Root nodes are created through the borrowed
//! [`RootHandle::scope`] capability and use the same node types as lexical
//! scopes.
//!
//! The runtime deliberately has no thread-local runtime-state fallback.
//! Computations are stored inside the state for their scope, and handles retain
//! a safe reference to their owning storage. Tracking execution contexts use a
//! thread-local stack only as a dynamic call boundary; every frame is bound to
//! its scheduler, so `untrack` cannot mask another runtime. User callbacks are
//! always invoked after the mutable state borrow has been released.
//!
//! Lexical handles cannot escape [`Scope::child`]. Root handles borrow stable
//! storage, so a root node cannot outlive the owner that stores it.

#![deny(unreachable_pub)]

mod child;
mod completion;
mod error;
mod handle;
mod internal;
mod root;
mod runtime;
mod scope;
mod unsafe_boundary;

pub use crate::{
    child::*,
    completion::{CompletionOnce, CompletionSender, unwind_safe},
    error::{
        CallbackInvokeError, CallbackInvokeResult, CompletionSubmitResult, ComputationInitError,
        ComputationInitResult, ErrorContext, ErrorHandler, HandlerError, ReactiveError,
        ReactiveResult,
    },
    handle::{
        CallbackId, DerivedId, EffectId, Handle, MemoId, NodeKind, NodeKindTag, NodeRefId,
        SignalId, StoredId, kind,
    },
    root::*,
    runtime::Runtime,
};

#[cfg(feature = "test-support")]
pub use runtime::RuntimeSnapshot;
