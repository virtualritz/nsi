//! Typed failure modes of the stream contract.
//!
//! Every failure mode named in `contracts/attribute-vocabulary.md` and
//! `contracts/publication-lifecycle.md` maps to exactly one [`enum@Error`]
//! variant. There is no silent fallback and no silent downgrade: a required
//! identifier that cannot be honored fails loudly (constitution principle V).

use thiserror::Error;

/// Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Typed stream failure.
///
/// The variants are the wire-visible failure modes of the version-1
/// contract. Adding a variant is a compatible change; changing the meaning
/// of one is not and requires a `stream.version` bump.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
#[non_exhaustive]
pub enum Error {
    /// A mandatory `stream.*` attribute was not set on the `outputdriver`
    /// node.
    #[error("required attribute `{name}` is missing")]
    MissingAttribute {
        /// Attribute name, as it appears on the `outputdriver` node.
        name: String,
    },

    /// A known attribute was present but carried the wrong ɴsɪ type or an
    /// out-of-range value.
    #[error("attribute `{name}` is malformed: {reason}")]
    MalformedAttribute {
        /// Attribute name, as it appears on the `outputdriver` node.
        name: String,
        /// Human-readable reason, naming the expected shape.
        reason: String,
    },

    /// `stream.version` named a vocabulary version this build does not
    /// implement.
    #[error(
        "`stream.version` {requested} is not supported (supported: {supported})"
    )]
    UnsupportedVersion {
        /// Version requested by the client.
        requested: i32,
        /// Version implemented by this build.
        supported: i32,
    },

    /// A transport was requested (explicitly, or as the last `"auto"`
    /// candidate) and is not viable.
    #[error("transport `{transport}` is unavailable: {reason}")]
    TransportUnavailable {
        /// Transport name as spelled in `stream.transport`.
        transport: String,
        /// Why the transport is not viable.
        reason: String,
    },

    /// `stream.device.uuid` named an adapter the driver does not render on.
    #[error(
        "device mismatch: `stream.device.uuid` requested `{requested}`, \
         driver renders on `{actual}`"
    )]
    DeviceMismatch {
        /// Adapter UUID requested by the client.
        requested: String,
        /// Adapter UUID the driver actually renders on.
        actual: String,
    },

    /// The `stream.channel` rendezvous peer is gone or the framing was
    /// violated.
    #[error("the rendezvous channel is closed")]
    ChannelClosed,

    /// A timeline wait expired before the publication's value was signaled.
    #[error("timed out waiting for timeline value {serial}")]
    WaitTimeout {
        /// The timeline value that was waited on. In the CPU ring this
        /// equals the publication's frame serial.
        serial: u64,
    },

    /// The stream was closed; no further publications are accepted.
    #[error("the stream is closed")]
    Closed,

    /// A bucket write did not fit the layer it was addressed to.
    #[error("bucket write rejected: {reason}")]
    InvalidWrite {
        /// Which part of the bucket description did not fit.
        reason: String,
    },

    /// An operating-system call failed.
    ///
    /// The error is flattened into strings so that [`enum@Error`] stays
    /// `Clone`, `Eq` and `Hash` like the rest of the public surface.
    #[error("{context}: {reason}")]
    Io {
        /// What the crate was doing.
        context: String,
        /// The operating-system error, rendered.
        reason: String,
    },
}

impl Error {
    /// Construct an [`Error::MalformedAttribute`].
    pub fn malformed(
        name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::MalformedAttribute {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Construct an [`Error::TransportUnavailable`].
    pub fn unavailable(
        transport: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::TransportUnavailable {
            transport: transport.into(),
            reason: reason.into(),
        }
    }

    /// Construct an [`Error::InvalidWrite`].
    pub fn invalid_write(reason: impl Into<String>) -> Self {
        Self::InvalidWrite {
            reason: reason.into(),
        }
    }

    /// Construct an [`Error::Io`] from anything that renders.
    pub fn io(context: impl Into<String>, reason: impl ToString) -> Self {
        Self::Io {
            context: context.into(),
            reason: reason.to_string(),
        }
    }
}
