use super::SilexError;
use silex_reactivity::CloseError;
use std::{error::Error, fmt};

/// Structured error for low-level DOM, backend and host operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomError {
    CrossContext {
        expected: u64,
        actual: u64,
    },
    InvalidHandle {
        backend: u64,
        kind: &'static str,
    },
    Detached {
        kind: &'static str,
    },
    WrongNodeKind {
        expected: &'static str,
        actual: &'static str,
    },
    CannotContain {
        parent: &'static str,
    },
    Cycle,
    CannotRemoveDocument,
    NoParent,
    ParentMismatch,
    ReferenceNotChild,
    AttributeNameEmpty,
    NodeRefBorrowed,
    NotBound,
    Cleared {
        generation: u64,
    },
    BindingGenerationExhausted,
    Unsupported {
        capability: &'static str,
    },
    Backend {
        operation: &'static str,
        message: String,
    },
}

pub type DomResult<T> = Result<T, DomError>;

impl fmt::Display for DomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrossContext { expected, actual } => write!(
                formatter,
                "DOM handles belong to different contexts (expected {expected}, got {actual})"
            ),
            Self::InvalidHandle { backend, kind } => {
                write!(formatter, "invalid {kind} handle for backend {backend}")
            }
            Self::Detached { kind } => write!(formatter, "the {kind} node is detached"),
            Self::WrongNodeKind { expected, actual } => {
                write!(formatter, "expected {expected} node, got {actual}")
            }
            Self::CannotContain { parent } => {
                write!(formatter, "{parent} cannot contain children")
            }
            Self::Cycle => formatter.write_str("the tree operation would create a cycle"),
            Self::CannotRemoveDocument => {
                formatter.write_str("the document node cannot be removed")
            }
            Self::NoParent => formatter.write_str("the node has no parent"),
            Self::ParentMismatch => formatter.write_str("the node does not belong to that parent"),
            Self::ReferenceNotChild => {
                formatter.write_str("the reference is not a child of the parent")
            }
            Self::AttributeNameEmpty => formatter.write_str("attribute or property name is empty"),
            Self::NodeRefBorrowed => formatter.write_str("the DOM NodeRef is already borrowed"),
            Self::NotBound => formatter.write_str("the DOM NodeRef has no binding"),
            Self::Cleared { generation } => {
                write!(
                    formatter,
                    "the DOM NodeRef binding was cleared (generation {generation})"
                )
            }
            Self::BindingGenerationExhausted => {
                formatter.write_str("the DOM NodeRef binding generation is exhausted")
            }
            Self::Unsupported { capability } => {
                write!(formatter, "unsupported capability: {capability}")
            }
            Self::Backend { operation, message } => {
                write!(formatter, "{operation} failed: {message}")
            }
        }
    }
}

impl Error for DomError {}

/// Identifies where a DOM cleanup failure occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupOrigin {
    Root,
    ProvisionalOwner,
    MountBoundary,
}

/// A cleanup failure annotated with its lifecycle boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupFailure {
    pub origin: CleanupOrigin,
    pub error: CloseError,
}

impl CleanupFailure {
    pub fn new(origin: CleanupOrigin, error: CloseError) -> Self {
        Self { origin, error }
    }

    pub fn into_parts(self) -> (CleanupOrigin, CloseError) {
        (self.origin, self.error)
    }
}

/// All cleanup and boundary errors collected during rollback or disposal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    cleanup_failures: Vec<CleanupFailure>,
    boundary_errors: Vec<SilexError>,
}

impl CleanupReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_parts(
        cleanup_failures: Vec<CleanupFailure>,
        boundary_errors: Vec<SilexError>,
    ) -> Self {
        Self {
            cleanup_failures,
            boundary_errors,
        }
    }

    pub fn is_clean(&self) -> bool {
        self.cleanup_failures.is_empty() && self.boundary_errors.is_empty()
    }

    pub fn cleanup_failures(&self) -> &[CleanupFailure] {
        &self.cleanup_failures
    }

    pub fn boundary_errors(&self) -> &[SilexError] {
        &self.boundary_errors
    }

    pub fn into_parts(self) -> (Vec<CleanupFailure>, Vec<SilexError>) {
        (self.cleanup_failures, self.boundary_errors)
    }
}
