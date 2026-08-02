//! Long-lived, owner-backed root scopes.

mod node;
mod scope;

pub use node::{
    RootCallback, RootDerived, RootEffect, RootMemo, RootNodeRef, RootReadSignal, RootSignal,
    RootStoredValue, RootWriteSignal,
};
pub use scope::{CleanupError, RootHandle, RootScope};
