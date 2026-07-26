//! Profile versioning and the `nsi-profile:` `shaderfilename` scheme.
//!
//! # Wire Format: `nsi-profile:<node>@<version>`
//!
//! A conforming scene addresses a profile node from a standard ɴsɪ `shader`
//! node by setting its `shaderfilename` attribute to a string of the form
//!
//! ```text
//! nsi-profile:<node>@<version>
//! ```
//!
//! where
//!
//! - `<node>` is a profile node name -- lowercase ASCII letters, digits and
//!   `_`, e.g. `diffuse_bsdf`.
//! - `<version>` is either `<major>` or `<major>.<minor>`, e.g. `1` or
//!   `1.0`. Patch levels are deliberately not addressable: they carry no
//!   vocabulary change.
//!
//! Examples: `nsi-profile:diffuse_bsdf@1`, `nsi-profile:image@1.0`.
//!
//! This is a wire format (constitution principle V). It introduces no new
//! ɴsɪ node types and no new API calls -- it is an ordinary string attribute
//! on an ordinary `shader` node, and a renderer that does not know the
//! profile sees a `shaderfilename` it cannot open, not a broken scene graph.
//!
//! # Compatibility And Versioning
//!
//! Profile versions are [semantic versions](semver). The rules are those of
//! `data-model.md`:
//!
//! - Additive changes -- new nodes, new closures, new *optional* ports --
//!   bump the **minor** version.
//! - Any change to existing semantics, port sets, or to the
//!   [`ParameterBlock`](crate::parameter_block) layout algorithm bumps the
//!   **major** version.
//! - A network that validates against version `N` must validate against any
//!   `N.x` with `x >= N.minor`. This is why resolution accepts a request
//!   whose minor is *less than or equal to* the registered profile minor,
//!   and rejects a request for a higher minor: the scene may rely on nodes
//!   the running profile does not have.
//!
//! # Failure Modes
//!
//! Resolution never falls back silently (constitution principle V). Every
//! failure is a typed [`ResolveError`]:
//!
//! | Situation | Error |
//! | --- | --- |
//! | `shaderfilename` is not `nsi-profile:`-prefixed (arbitrary ᴏsʟ) | [`ResolveError::NotProfileScheme`] |
//! | Prefix present but the remainder does not parse | [`ResolveError::MalformedScheme`] |
//! | Well-formed reference to a node this profile does not define | [`ResolveError::UnknownNode`] |
//! | Well-formed reference to a version no registered profile provides | [`ResolveError::UnsupportedVersion`] |
use core::fmt;

pub use semver::Version;

use crate::error::ResolveError;

/// The `shaderfilename` scheme prefix, including the colon.
pub const SCHEME_PREFIX: &str = "nsi-profile:";

/// Profile version 1.0.0 -- the vocabulary frozen by feature
/// `002-shading-profile`.
pub const PROFILE_V1: Version = Version::new(1, 0, 0);

/// A profile version as *requested* by a `shaderfilename`.
///
/// The minor component is optional: `@1` requests "any 1.x this
/// implementation provides", `@1.2` requests "at least 1.2".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestedVersion {
    /// The major version. Must match the profile exactly.
    pub major: u64,
    /// The minor version, if the reference pinned one.
    pub minor: Option<u64>,
}

impl RequestedVersion {
    /// Returns whether `profile` satisfies this request.
    ///
    /// The major version must match exactly; a pinned minor must not exceed
    /// the profile minor.
    #[must_use]
    pub fn is_satisfied_by(&self, profile: &Version) -> bool {
        self.major == profile.major
            && self.minor.is_none_or(|minor| minor <= profile.minor)
    }
}

impl fmt::Display for RequestedVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.minor {
            Some(minor) => write!(f, "{}.{}", self.major, minor),
            None => write!(f, "{}", self.major),
        }
    }
}

/// A parsed `nsi-profile:<node>@<version>` reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemeRef<'a> {
    /// The profile node name.
    pub node: &'a str,
    /// The requested profile version.
    pub version: RequestedVersion,
}

impl fmt::Display for SchemeRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{SCHEME_PREFIX}{}@{}", self.node, self.version)
    }
}

/// Returns whether `shaderfilename` uses the profile scheme at all.
///
/// A `false` here is what separates "arbitrary ᴏsʟ" from "profile node"; the
/// validator reports the former as a violation rather than guessing.
#[must_use]
pub fn is_profile_scheme(shaderfilename: &str) -> bool {
    shaderfilename.starts_with(SCHEME_PREFIX)
}

/// Parses a `shaderfilename` into a [`SchemeRef`].
///
/// # Errors
///
/// - [`ResolveError::NotProfileScheme`] if the string does not start with
///   `nsi-profile:`.
/// - [`ResolveError::MalformedScheme`] if the remainder is not
///   `<node>@<major>[.<minor>]` with a syntactically valid node name.
pub fn parse_scheme(
    shaderfilename: &str,
) -> Result<SchemeRef<'_>, ResolveError> {
    let rest = shaderfilename.strip_prefix(SCHEME_PREFIX).ok_or_else(|| {
        ResolveError::NotProfileScheme {
            shaderfilename: shaderfilename.to_string(),
        }
    })?;

    let malformed = |reason: &str| ResolveError::MalformedScheme {
        shaderfilename: shaderfilename.to_string(),
        reason: reason.to_string(),
    };

    let (node, version) = rest
        .split_once('@')
        .ok_or_else(|| malformed("missing `@<version>` suffix"))?;

    if node.is_empty() {
        Err(malformed("empty node name"))
    } else if !node
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        Err(malformed(
            "node name must be lowercase ASCII letters, digits or `_`",
        ))
    } else if version.contains('@') {
        Err(malformed("more than one `@` separator"))
    } else {
        parse_version(version)
            .map(|version| SchemeRef { node, version })
            .ok_or_else(|| {
                malformed("version must be `<major>` or `<major>.<minor>`")
            })
    }
}

/// Parses the `<major>[.<minor>]` tail of a scheme reference.
fn parse_version(version: &str) -> Option<RequestedVersion> {
    let digits = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u64>().ok())
            .flatten()
    };

    match version.split_once('.') {
        None => {
            digits(version).map(|major| RequestedVersion { major, minor: None })
        }
        Some((major, minor)) => {
            digits(major).zip(digits(minor)).map(|(major, minor)| {
                RequestedVersion {
                    major,
                    minor: Some(minor),
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RequestedVersion, parse_scheme};
    use crate::error::ResolveError;

    #[test]
    fn parses_major_only() {
        let reference = parse_scheme("nsi-profile:diffuse_bsdf@1").unwrap();
        assert_eq!(reference.node, "diffuse_bsdf");
        assert_eq!(
            reference.version,
            RequestedVersion {
                major: 1,
                minor: None
            }
        );
    }

    #[test]
    fn parses_major_minor() {
        let reference = parse_scheme("nsi-profile:image@1.0").unwrap();
        assert_eq!(
            reference.version,
            RequestedVersion {
                major: 1,
                minor: Some(0)
            }
        );
    }

    #[test]
    fn rejects_foreign_scheme() {
        assert!(matches!(
            parse_scheme("shaders/custom.oso"),
            Err(ResolveError::NotProfileScheme { .. })
        ));
    }

    #[test]
    fn rejects_missing_version() {
        assert!(matches!(
            parse_scheme("nsi-profile:diffuse_bsdf"),
            Err(ResolveError::MalformedScheme { .. })
        ));
    }
}
