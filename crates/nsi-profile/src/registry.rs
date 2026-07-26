//! The profile registry -- what a given profile version contains, and how a
//! `shaderfilename` resolves against it.
use crate::{
    closure::{ClosureDef, V1_CLOSURES},
    error::ResolveError,
    node::NodeDef,
    v1::V1_NODES,
    version::{PROFILE_V1, RequestedVersion, Version, parse_scheme},
};

/// One profile version: the definitive node and closure tables for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    version: Version,
    nodes: &'static [NodeDef],
    closures: &'static [ClosureDef],
}

impl Profile {
    /// Profile v1 -- the vocabulary frozen by feature `002-shading-profile`.
    pub const V1: Self = Self {
        version: PROFILE_V1,
        nodes: V1_NODES,
        closures: V1_CLOSURES,
    };

    /// Builds a profile from explicit tables.
    #[must_use]
    pub const fn new(
        version: Version,
        nodes: &'static [NodeDef],
        closures: &'static [ClosureDef],
    ) -> Self {
        Self {
            version,
            nodes,
            closures,
        }
    }

    /// The closure table, in canonical order.
    #[must_use]
    pub const fn closures(&self) -> &'static [ClosureDef] {
        self.closures
    }

    /// The node table, in canonical order.
    #[must_use]
    pub const fn nodes(&self) -> &'static [NodeDef] {
        self.nodes
    }

    /// This profile's version.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }

    /// Looks a closure up by name.
    #[must_use]
    pub fn closure(&self, name: &str) -> Option<&'static ClosureDef> {
        self.closures.iter().find(|closure| closure.name == name)
    }

    /// Looks a node up by name.
    #[must_use]
    pub fn node(&self, name: &str) -> Option<&'static NodeDef> {
        self.nodes.iter().find(|node| node.name == name)
    }
}

/// A successfully resolved `nsi-profile:<node>@<version>` reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved<'a> {
    /// The node definition the reference names.
    pub node: &'static NodeDef,
    /// The profile whose table was consulted.
    pub profile: &'a Profile,
}

/// The set of profile versions an implementation provides.
///
/// Resolution picks the profile whose major version matches the request; see
/// the [`version`](crate::version) module for the compatibility rules.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Registry {
    profiles: Vec<Profile>,
}

impl Registry {
    /// A registry providing only profile v1.
    #[must_use]
    pub fn v1() -> Self {
        Self::new(vec![Profile::V1])
    }

    /// Builds a registry from an explicit profile list.
    ///
    /// Profiles are stored newest-first, so that
    /// [`latest`](Self::latest) and the `available` field of
    /// [`ResolveError::UnsupportedVersion`] are deterministic regardless of
    /// the order they were passed in.
    #[must_use]
    pub fn new(mut profiles: Vec<Profile>) -> Self {
        profiles.sort_by(|a, b| b.version.cmp(&a.version));
        Self { profiles }
    }

    /// The highest registered profile, if any.
    #[must_use]
    pub fn latest(&self) -> Option<&Profile> {
        self.profiles.first()
    }

    /// The highest registered version, or `0.0.0` for an empty registry.
    #[must_use]
    pub fn latest_version(&self) -> Version {
        self.latest().map_or_else(
            || Version::new(0, 0, 0),
            |profile| profile.version.clone(),
        )
    }

    /// The profile with exactly this version.
    #[must_use]
    pub fn profile(&self, version: &Version) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|profile| &profile.version == version)
    }

    /// The highest profile satisfying `request`.
    #[must_use]
    pub fn profile_for(&self, request: &RequestedVersion) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|profile| request.is_satisfied_by(&profile.version))
    }

    /// All registered profiles, newest first.
    #[must_use]
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// Resolves a `shaderfilename` to a node definition.
    ///
    /// # Errors
    ///
    /// Every failure is typed and loud -- there is no fallback to a
    /// nearest-match node or version. See [`ResolveError`].
    pub fn resolve(
        &self,
        shaderfilename: &str,
    ) -> Result<Resolved<'_>, ResolveError> {
        let reference = parse_scheme(shaderfilename)?;

        let profile =
            self.profile_for(&reference.version).ok_or_else(|| {
                ResolveError::UnsupportedVersion {
                    requested: reference.version.to_string(),
                    available: self.latest_version(),
                }
            })?;

        profile
            .node(reference.node)
            .map(|node| Resolved { node, profile })
            .ok_or_else(|| ResolveError::UnknownNode {
                node: reference.node.to_string(),
                version_consulted: profile.version.clone(),
            })
    }
}
