//! Explicit, single-threaded scoped reactivity runtime.
//!
//! A user owns a [`Runtime`] and creates a persistent [`OwnerHandle`] or a
//! callback-scoped transient owner. Typed node payloads stay tied to the
//! [`OwnerAccess`] lifetime.
//!
//! The runtime deliberately has no thread-local runtime-state fallback.
//! Computations are stored inside the state for their scope, and handles retain
//! a safe reference to their owning storage. Tracking execution contexts use a
//! thread-local stack only as a dynamic call boundary; every frame is bound to
//! its scheduler, so `untrack` cannot mask another runtime. User callbacks are
//! always invoked after the mutable state borrow has been released.
//!
//! Transient owner access cannot escape its higher-ranked callback. A
//! persistent owner can be explicitly closed and is also closed best-effort
//! on drop.

#![deny(unreachable_pub)]

mod borrow;
mod completion;
mod error;
mod handle;
mod internal;
mod owner;
mod root;
mod runtime;
mod unsafe_boundary;

pub use crate::{
    completion::{CompletionOnce, CompletionSender, unwind_safe},
    error::{
        CallbackInvokeError, CallbackInvokeResult, CompletionSubmitError, CompletionSubmitResult,
        ComputationInitError, ComputationInitResult, ErrorContext, ErrorHandlerAnchor,
        ErrorHandlerInput, ErrorHandlerRef, ErrorHandlerToken, HandlerError, HandlerLease,
        HandlerReason, ReactiveError, ReactiveResult,
    },
    owner::{
        Callback, Computed, EffectHandle, EffectPhase, NodeRef, ReadGuard, ReadSignal, Signal,
        StoredValue, WatchOptions, WriteGuard, WriteSignal,
    },
    owner::{OwnerAccess, OwnerChild, OwnerCleanupRegistrationError, OwnerHandle},
    root::*,
    runtime::Runtime,
};

#[cfg(feature = "test-support")]
pub use runtime::RuntimeSnapshot;
