//! The loud validator (US3).
//!
//! [`validate`] answers one question: is this shader network inside the
//! named profile version? A conforming network yields an empty report; a
//! non-conforming one yields a [`Violation`] per problem, each naming the
//! **node handle**, the **construct**, and the **profile version
//! consulted** -- the three things US3 requires.
//!
//! The excluded-construct list in `spec.md` Non-Goals is normative: the
//! validator **rejects**, it never silently strips. A `shaderfilename` that
//! does not use the `nsi-profile:` scheme is arbitrary ᴏsʟ -- which may well
//! call `trace()`, `getattribute()` or manipulate strings -- and is reported
//! as the construct
//! [`NON_PROFILE_SHADERFILENAME`](construct::NON_PROFILE_SHADERFILENAME)
//! rather than inspected, guessed at, or dropped.
//!
//! Validation is not optional in the pipeline:
//! [`translate`](crate::translate::translate) runs it internally and refuses
//! a network with any violation.
use core::fmt;

use crate::{
    error::{Error, ResolveError},
    network::{Network, ParamValue},
    node::NodeDef,
    registry::Registry,
    version::{Version, parse_scheme},
};

/// The stable construct names a [`Violation`] can carry.
///
/// These strings are part of the validator's output contract: CI rules and
/// tests match on them, so they are versioned with the profile.
pub mod construct {
    /// The `shaderfilename` does not use the `nsi-profile:` scheme.
    pub const NON_PROFILE_SHADERFILENAME: &str = "non-profile shaderfilename";
    /// The `shaderfilename` uses the scheme but does not parse.
    pub const MALFORMED_SCHEME: &str = "malformed profile shaderfilename";
    /// The node name is not in the consulted profile.
    pub const UNKNOWN_NODE: &str = "unknown profile node";
    /// The requested version is not the consulted one.
    pub const UNSUPPORTED_VERSION: &str = "unsupported profile version";
    /// Two `shader` nodes share a handle.
    pub const DUPLICATE_HANDLE: &str = "duplicate node handle";
    /// A parameter name is not a port of the node.
    pub const UNKNOWN_PARAMETER: &str = "unknown parameter";
    /// A parameter value has the wrong type for its port.
    pub const PARAMETER_TYPE_MISMATCH: &str = "parameter type mismatch";
    /// A string parameter is not one of the port's allowed constants.
    pub const INVALID_ENUMERANT: &str = "invalid enumerant";
    /// A connection names an upstream handle that does not exist.
    pub const CONNECTION_FROM_UNKNOWN_NODE: &str =
        "connection from unknown node";
    /// A connection names a downstream handle that does not exist.
    pub const CONNECTION_TO_UNKNOWN_NODE: &str = "connection to unknown node";
    /// A connection names an output port the upstream node does not have.
    pub const CONNECTION_FROM_UNKNOWN_PORT: &str =
        "connection from nonexistent port";
    /// A connection names an input port the downstream node does not have.
    pub const CONNECTION_TO_UNKNOWN_PORT: &str =
        "connection to nonexistent port";
    /// The two ports of a connection have different types.
    pub const PORT_TYPE_MISMATCH: &str = "port type mismatch";
    /// Two connections feed the same input port.
    pub const DUPLICATE_CONNECTION: &str = "duplicate connection to port";
    /// The connections form a cycle.
    pub const CYCLE: &str = "cycle";
}

/// One out-of-profile construct found in a network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Violation {
    /// The ɴsɪ handle of the offending `shader` node.
    pub node_handle: String,
    /// What is out of profile, from [`construct`].
    pub construct: String,
    /// The profile version the validator consulted.
    pub version_consulted: Version,
    /// The specifics -- names, types, values.
    pub detail: String,
}

impl Violation {
    /// Builds a violation.
    #[must_use]
    pub fn new(
        node_handle: impl Into<String>,
        construct: &str,
        version_consulted: &Version,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            node_handle: node_handle.into(),
            construct: construct.to_string(),
            version_consulted: version_consulted.clone(),
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}`: {} -- {} (profile version {} consulted)",
            self.node_handle,
            self.construct,
            self.detail,
            self.version_consulted
        )
    }
}

/// The result of validating one network.
///
/// The [`Display`](fmt::Display) impl is the CI-log form: a header line plus
/// one indented line per violation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValidationReport {
    version_consulted: Version,
    violations: Vec<Violation>,
}

impl ValidationReport {
    /// Returns whether the network is inside the profile.
    #[must_use]
    pub fn is_conforming(&self) -> bool {
        self.violations.is_empty()
    }

    /// The violations, in a deterministic order: node problems in node
    /// declaration order, then parameter problems, then connection problems
    /// in connection order, then the cycle report.
    #[must_use]
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }

    /// The profile version consulted.
    #[must_use]
    pub const fn version_consulted(&self) -> &Version {
        &self.version_consulted
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.violations.is_empty() {
            write!(f, "profile {}: conforming", self.version_consulted)
        } else {
            writeln!(
                f,
                "profile {}: {} violation(s)",
                self.version_consulted,
                self.violations.len()
            )?;

            let last = self.violations.len() - 1;

            self.violations.iter().enumerate().try_for_each(
                |(index, violation)| {
                    if index == last {
                        write!(f, "  {violation}")
                    } else {
                        writeln!(f, "  {violation}")
                    }
                },
            )
        }
    }
}

/// Validates a network against one profile version.
///
/// `version` is the version consulted, and is reported verbatim on every
/// violation. If the registry does not provide it, *every* node is reported
/// as [`UNSUPPORTED_VERSION`](construct::UNSUPPORTED_VERSION): an
/// implementation that cannot even name the vocabulary must not claim a
/// network conforms to it.
#[must_use]
pub fn validate(
    network: &Network,
    registry: &Registry,
    version: &Version,
) -> ValidationReport {
    let violations = match registry.profile(version) {
        None => unregistered_version_violations(network, registry, version),
        Some(profile) => {
            let mut violations = Vec::new();
            let definitions = resolve_nodes(
                network,
                profile.nodes(),
                version,
                &mut violations,
            );

            check_params(network, &definitions, version, &mut violations);
            check_connections(network, &definitions, version, &mut violations);

            if let Err(Error::Cycle { handle }) = network.topological_order() {
                violations.push(Violation::new(
                    handle.as_str(),
                    construct::CYCLE,
                    version,
                    format!("connections form a cycle involving `{handle}`"),
                ));
            }

            violations
        }
    };

    ValidationReport {
        version_consulted: version.clone(),
        violations,
    }
}

/// Every node is a violation when the requested version does not exist.
fn unregistered_version_violations(
    network: &Network,
    registry: &Registry,
    version: &Version,
) -> Vec<Violation> {
    let available = registry.latest_version();

    network
        .nodes()
        .iter()
        .map(|node| {
            Violation::new(
                node.handle.as_str(),
                construct::UNSUPPORTED_VERSION,
                version,
                format!(
                    "profile version {version} is not registered (this implementation provides {available})"
                ),
            )
        })
        .collect()
}

/// Resolves every node's `shaderfilename`, recording a violation per
/// failure. The returned vector is parallel to `network.nodes()`.
fn resolve_nodes(
    network: &Network,
    nodes: &'static [NodeDef],
    version: &Version,
    violations: &mut Vec<Violation>,
) -> Vec<Option<&'static NodeDef>> {
    network
        .nodes()
        .iter()
        .enumerate()
        .map(|(index, node)| {
            if network.nodes()[..index]
                .iter()
                .any(|earlier| earlier.handle == node.handle)
            {
                violations.push(Violation::new(
                    node.handle.as_str(),
                    construct::DUPLICATE_HANDLE,
                    version,
                    format!(
                        "handle `{}` is declared more than once",
                        node.handle
                    ),
                ));
            }

            match parse_scheme(&node.shaderfilename) {
                Err(ResolveError::NotProfileScheme { shaderfilename }) => {
                    violations.push(Violation::new(
                        node.handle.as_str(),
                        construct::NON_PROFILE_SHADERFILENAME,
                        version,
                        format!(
                            "`{shaderfilename}` is arbitrary ᴏsʟ, not an `nsi-profile:` node"
                        ),
                    ));
                    None
                }
                Err(error) => {
                    violations.push(Violation::new(
                        node.handle.as_str(),
                        construct::MALFORMED_SCHEME,
                        version,
                        error.to_string(),
                    ));
                    None
                }
                Ok(reference) if !reference.version.is_satisfied_by(version) => {
                    violations.push(Violation::new(
                        node.handle.as_str(),
                        construct::UNSUPPORTED_VERSION,
                        version,
                        format!(
                            "`{}` requests profile version {}",
                            node.shaderfilename, reference.version
                        ),
                    ));
                    None
                }
                Ok(reference) => {
                    let definition = nodes
                        .iter()
                        .find(|definition| definition.name == reference.node);

                    if definition.is_none() {
                        violations.push(Violation::new(
                            node.handle.as_str(),
                            construct::UNKNOWN_NODE,
                            version,
                            format!(
                                "`{}` is not a node of profile {version}",
                                reference.node
                            ),
                        ));
                    }

                    definition
                }
            }
        })
        .collect()
}

/// Checks every parameter against its port.
fn check_params(
    network: &Network,
    definitions: &[Option<&'static NodeDef>],
    version: &Version,
    violations: &mut Vec<Violation>,
) {
    network
        .nodes()
        .iter()
        .zip(definitions.iter().copied())
        .filter_map(|(node, definition)| {
            definition.map(|definition| (node, definition))
        })
        .for_each(|(node, definition)| {
            node.params.iter().for_each(|(name, value)| {
                match definition.input(name) {
                    None => violations.push(Violation::new(
                        node.handle.as_str(),
                        construct::UNKNOWN_PARAMETER,
                        version,
                        format!(
                            "`{name}` is not an input of profile node `{}`",
                            definition.name
                        ),
                    )),
                    Some(port) if port.ty != value.port_type() => violations
                        .push(Violation::new(
                            node.handle.as_str(),
                            construct::PARAMETER_TYPE_MISMATCH,
                            version,
                            format!(
                                "`{name}` is `{}`, value is `{}`",
                                port.ty,
                                value.port_type()
                            ),
                        )),
                    Some(port) => {
                        if let ParamValue::String(text) = value
                            && !port.allowed.is_empty()
                            && !port.allowed.contains(&text.as_str())
                        {
                            violations.push(Violation::new(
                                node.handle.as_str(),
                                construct::INVALID_ENUMERANT,
                                version,
                                format!(
                                    "`{name}` is `{text}`, expected one of {:?}",
                                    port.allowed
                                ),
                            ));
                        }
                    }
                }
            });
        });
}

/// Checks every connection's endpoints and port types.
fn check_connections(
    network: &Network,
    definitions: &[Option<&'static NodeDef>],
    version: &Version,
    violations: &mut Vec<Violation>,
) {
    network
        .connections()
        .iter()
        .enumerate()
        .for_each(|(index, connection)| {
            let describe = format!(
                "`{}.{}` -> `{}.{}`",
                connection.from_handle,
                connection.from_output,
                connection.to_handle,
                connection.to_input
            );

            let from = network
                .node_index(&connection.from_handle)
                .and_then(|index| definitions[index]);
            let to = network
                .node_index(&connection.to_handle)
                .and_then(|index| definitions[index]);

            if network.node(&connection.from_handle).is_none() {
                violations.push(Violation::new(
                    connection.to_handle.as_str(),
                    construct::CONNECTION_FROM_UNKNOWN_NODE,
                    version,
                    describe.clone(),
                ));
            }

            if network.node(&connection.to_handle).is_none() {
                violations.push(Violation::new(
                    connection.to_handle.as_str(),
                    construct::CONNECTION_TO_UNKNOWN_NODE,
                    version,
                    describe.clone(),
                ));
            }

            let output = from.and_then(|definition| {
                let port = definition.output(&connection.from_output);

                if port.is_none() {
                    violations.push(Violation::new(
                        connection.to_handle.as_str(),
                        construct::CONNECTION_FROM_UNKNOWN_PORT,
                        version,
                        format!(
                            "{describe}: `{}` has no output `{}`",
                            definition.name, connection.from_output
                        ),
                    ));
                }

                port
            });

            let input = to.and_then(|definition| {
                let port = definition.input(&connection.to_input);

                if port.is_none() {
                    violations.push(Violation::new(
                        connection.to_handle.as_str(),
                        construct::CONNECTION_TO_UNKNOWN_PORT,
                        version,
                        format!(
                            "{describe}: `{}` has no input `{}`",
                            definition.name, connection.to_input
                        ),
                    ));
                }

                port
            });

            if let Some((output, input)) = output.zip(input)
                && output.ty != input.ty
            {
                violations.push(Violation::new(
                    connection.to_handle.as_str(),
                    construct::PORT_TYPE_MISMATCH,
                    version,
                    format!(
                        "{describe}: `{}` is `{}`, `{}` is `{}`",
                        connection.from_output,
                        output.ty,
                        connection.to_input,
                        input.ty
                    ),
                ));
            }

            if network.connections()[..index].iter().any(|earlier| {
                earlier.to_handle == connection.to_handle
                    && earlier.to_input == connection.to_input
            }) {
                violations.push(Violation::new(
                    connection.to_handle.as_str(),
                    construct::DUPLICATE_CONNECTION,
                    version,
                    format!(
                        "{describe}: `{}` is already connected",
                        connection.to_input
                    ),
                ));
            }
        });
}
