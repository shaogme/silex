//! Long-lived root owner.

mod scope;

pub use scope::{
    CleanupDiagnostic, CleanupFailure, CleanupPayloadKind, CloseError, CloseFailure, ClosePhase,
    CloseSource, CloseTransaction,
};
