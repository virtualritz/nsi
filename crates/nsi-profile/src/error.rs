//! Typed failure modes.
//!
//! The profile never falls back silently (constitution principle V): an
//! unknown node, an unsupported version, an out-of-profile construct or a
//! malformed edit is always an error value, never a default substitution.
use thiserror::Error;

use crate::{node::PortType, validate::ValidationReport, version::Version};

/// Failure of `nsi-profile:<node>@<version>` resolution.
///
/// See the [`version`](crate::version) module for the scheme grammar and the
/// compatibility rules these errors enforce.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
pub enum ResolveError {
    /// The `shaderfilename` does not use the `nsi-profile:` scheme at all --
    /// it addresses arbitrary ᴏsʟ, which is outside every profile version.
    #[error(
        "`{shaderfilename}` is not a profile `shaderfilename` (expected the `nsi-profile:` scheme)"
    )]
    NotProfileScheme {
        /// The offending `shaderfilename` attribute value.
        shaderfilename: String,
    },

    /// The `nsi-profile:` prefix is present but the remainder does not parse
    /// as `<node>@<major>[.<minor>]`.
    #[error("malformed profile `shaderfilename` `{shaderfilename}`: {reason}")]
    MalformedScheme {
        /// The offending `shaderfilename` attribute value.
        shaderfilename: String,
        /// Why it failed to parse.
        reason: String,
    },

    /// The reference is well-formed and the version exists, but the profile
    /// of that version defines no such node.
    #[error(
        "unknown profile node `{node}` in profile version {version_consulted}"
    )]
    UnknownNode {
        /// The requested node name.
        node: String,
        /// The profile version whose node table was consulted.
        version_consulted: Version,
    },

    /// No registered profile satisfies the requested version.
    #[error(
        "unsupported profile version `{requested}` (this implementation provides {available})"
    )]
    UnsupportedVersion {
        /// The version as written in the `shaderfilename`.
        requested: String,
        /// The highest profile version this implementation provides.
        available: Version,
    },
}

/// Failure of validation, translation, parameter-block access or emission.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// A `shaderfilename` failed to resolve.
    #[error(transparent)]
    Resolve(#[from] ResolveError),

    /// The network does not conform to the profile.
    ///
    /// Translation runs validation internally and refuses non-conforming
    /// input; validation is not an optional pipeline stage
    /// (`contracts/profile-conformance.md`, failure modes).
    #[error("shader network does not conform to profile {version}:\n{report}")]
    NotConforming {
        /// The profile version consulted.
        version: Version,
        /// The full report, one line per violation.
        report: ValidationReport,
    },

    /// The network's connections form a cycle.
    #[error("shader network contains a cycle involving node `{handle}`")]
    Cycle {
        /// A node handle on the cycle.
        handle: String,
    },

    /// The network has no unconnected `Surface`-typed output to translate.
    #[error(
        "shader network has no terminal node (no unconnected `Surface` output)"
    )]
    MissingTerminal,

    /// More than one candidate terminal -- the network root is ambiguous.
    #[error(
        "shader network has more than one terminal node: {handles:?}; exactly one unconnected `Surface` output is required"
    )]
    AmbiguousTerminal {
        /// The competing terminal handles, in topological order.
        handles: Vec<String>,
    },

    /// A parameter write targeted something the parameter block does not
    /// carry -- a connected port, a string parameter, or an unknown name.
    ///
    /// This is not a fallback point: the caller must re-translate instead.
    #[error(
        "`{handle}.{param}` is not a parameter-block field; it requires re-translation"
    )]
    NotABlockParameter {
        /// The shader node handle.
        handle: String,
        /// The parameter name.
        param: String,
    },

    /// A parameter write supplied a value of the wrong type.
    #[error(
        "type mismatch writing `{handle}.{param}`: field is `{expected}`, value is `{found}`"
    )]
    ParameterTypeMismatch {
        /// The shader node handle.
        handle: String,
        /// The parameter name.
        param: String,
        /// The declared field type.
        expected: PortType,
        /// The type of the supplied value.
        found: PortType,
    },

    /// The destination buffer is smaller than the parameter block.
    #[error("parameter buffer too small: need {needed} bytes, got {got}")]
    BufferTooSmall {
        /// Bytes required.
        needed: usize,
        /// Bytes offered.
        got: usize,
    },
}
