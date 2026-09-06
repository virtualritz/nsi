//! The error a recording call can fail with.

use core::fmt;

/// Why an ɴsɪ call could not be recorded.
///
/// One type across every [`Nsi`] method, so a consumer matches on one
/// thing. It is `#[non_exhaustive]`: ɴsɪ has more rules than this crate
/// enforces yet, and enforcing another one should not be a breaking
/// change.
///
/// [`Nsi`]: nsi_trait::Nsi
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RecordError {
    /// A call tried to create or delete one of ɴsɪ's reserved nodes.
    ///
    /// ɴsɪ: "it is not possible to delete the root or the global node",
    /// and they "don't need to be created" because they already exist.
    Reserved {
        /// The reserved handle.
        handle: String,
    },
    /// A `connect` or `disconnect` named a handle that does not exist.
    ///
    /// ɴsɪ: "the nodes on which the connection is performed must
    /// exist." Recording the edge anyway builds a graph whose nodes are
    /// missing, and resolution then answers for it as though it were
    /// real.
    UnknownHandle {
        /// The handle that was never created.
        handle: String,
    },
    /// `create` was called again for an existing handle with a
    /// different node type.
    ///
    /// ɴsɪ: "the function does nothing if all other parameters match the
    /// call which created that node. Otherwise, it emits an error."
    /// Overwriting the type silently would leave the scene describing
    /// something no consumer asked for.
    TypeMismatch {
        /// The handle that already exists.
        handle: String,
        /// The type it was created with.
        existing: String,
        /// The type this call asked for.
        requested: String,
    },
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved { handle } => write!(
                f,
                "ɴsɪ node {handle:?} is reserved: it exists already and \
                 cannot be created or deleted"
            ),
            Self::UnknownHandle { handle } => write!(
                f,
                "ɴsɪ connection names node {handle:?}, which does not \
                 exist; the nodes a connection is made on must be \
                 created first"
            ),
            Self::TypeMismatch {
                handle,
                existing,
                requested,
            } => write!(
                f,
                "ɴsɪ node {handle:?} already exists as {existing:?}; \
                 re-creating it as {requested:?} is an error, not an \
                 overwrite"
            ),
        }
    }
}

impl core::error::Error for RecordError {}
