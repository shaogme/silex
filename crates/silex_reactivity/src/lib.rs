//! Explicit, single-threaded scoped reactivity runtime.
//!
//! A user owns a [`Runtime`] and enters it with [`Runtime::run`]. Every node is
//! created through the [`Scope`] passed to that run and carries a lifetime tied
//! to the scope. Child scopes are lexical: their nodes are destroyed before the
//! child callback returns.
//!
//! The runtime deliberately has no thread-local fallback. Computations are
//! stored as `Box<dyn FnMut() + 'scope>` inside the state for their scope, and
//! handles retain only safe weak state references. User callbacks are always
//! invoked after the mutable state borrow has been released.
//!
//! ```compile_fail
//! use silex_reactivity::Runtime;
//!
//! let mut runtime = Runtime::new();
//! let _signal = runtime.run(|scope| scope.signal(0i32).0);
//! ```

#![deny(unreachable_pub)]

mod error;
mod handle;
mod internal;
pub mod node;
mod runtime;
pub mod scope;

pub use crate::{
    error::{ReactiveError, ReactiveResult},
    handle::{
        AnyHandle, CallbackId, DerivedId, EffectId, Handle, MemoId, NodeKind, NodeKindTag,
        NodeRefId, SignalId, StoredId, kind,
    },
    internal::RawId,
    node::{
        Callback, Derived, Effect, Memo, NodeRef, ReadSignal, RwSignal, Signal, StoredValue,
        WriteSignal,
    },
    runtime::Runtime,
    scope::Scope,
};
