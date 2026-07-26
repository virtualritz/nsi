//! The ɴsɪ-side shader graph.
//!
//! A [`Network`] mirrors exactly what a conforming scene already contains:
//! standard `shader` nodes whose `shaderfilename` attribute uses the
//! `nsi-profile:` scheme, ordinary attributes for their parameters, and
//! ordinary `NSIConnect` calls between them. No new node types and no new
//! API calls are introduced (contract invariant, requirement R7).
use crate::{error::Error, node::PortType};

/// A parameter value as an ɴsɪ scene would set it.
///
/// This is the small subset of the ɴsɪ type system a profile `shader` node
/// can carry -- deliberately not a general `NSIParam_t`: array-valued and
/// per-vertex attributes are not profile parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// A `float`.
    Float(f32),
    /// An `int`.
    Int(i32),
    /// A linear-RGB triple.
    Color([f32; 3]),
    /// A direction triple.
    Vector([f32; 3]),
    /// A normal triple.
    Normal([f32; 3]),
    /// A position triple.
    Point([f32; 3]),
    /// A string -- an enumerant or a resource name.
    String(String),
}

impl ParamValue {
    /// The three components, for triple-typed values.
    #[must_use]
    pub fn as_triple(&self) -> Option<[f32; 3]> {
        match self {
            Self::Color(v)
            | Self::Vector(v)
            | Self::Normal(v)
            | Self::Point(v) => Some(*v),
            Self::Float(_) | Self::Int(_) | Self::String(_) => None,
        }
    }

    /// The port type this value can feed.
    #[must_use]
    pub const fn port_type(&self) -> PortType {
        match self {
            Self::Float(_) => PortType::Float,
            Self::Int(_) => PortType::Int,
            Self::Color(_) => PortType::Color,
            Self::Vector(_) => PortType::Vector,
            Self::Normal(_) => PortType::Normal,
            Self::Point(_) => PortType::Point,
            Self::String(_) => PortType::String,
        }
    }
}

/// One ɴsɪ `shader` node.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderNode {
    /// The ɴsɪ node handle.
    pub handle: String,
    /// The `shaderfilename` attribute.
    pub shaderfilename: String,
    /// Parameters set on the node, in the order the scene set them.
    pub params: Vec<(String, ParamValue)>,
}

impl ShaderNode {
    /// A `shader` node with no parameters set.
    #[must_use]
    pub fn new(
        handle: impl Into<String>,
        shaderfilename: impl Into<String>,
    ) -> Self {
        Self {
            handle: handle.into(),
            shaderfilename: shaderfilename.into(),
            params: Vec::new(),
        }
    }

    /// Looks a parameter up by name.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&ParamValue> {
        self.params
            .iter()
            .find(|(param, _)| param == name)
            .map(|(_, value)| value)
    }

    /// Sets a parameter, builder style.
    #[must_use]
    pub fn with_param(
        mut self,
        name: impl Into<String>,
        value: ParamValue,
    ) -> Self {
        self.params.push((name.into(), value));
        self
    }
}

/// One `NSIConnect` between two `shader` nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Connection {
    /// Handle of the upstream node.
    pub from_handle: String,
    /// Output port on the upstream node.
    pub from_output: String,
    /// Handle of the downstream node.
    pub to_handle: String,
    /// Input port on the downstream node.
    pub to_input: String,
}

impl Connection {
    /// A connection between two ports.
    #[must_use]
    pub fn new(
        from_handle: impl Into<String>,
        from_output: impl Into<String>,
        to_handle: impl Into<String>,
        to_input: impl Into<String>,
    ) -> Self {
        Self {
            from_handle: from_handle.into(),
            from_output: from_output.into(),
            to_handle: to_handle.into(),
            to_input: to_input.into(),
        }
    }
}

/// A shader network: `shader` nodes plus the connections between them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Network {
    nodes: Vec<ShaderNode>,
    connections: Vec<Connection>,
}

impl Network {
    /// Builds a network from nodes and connections.
    #[must_use]
    pub fn new(nodes: Vec<ShaderNode>, connections: Vec<Connection>) -> Self {
        Self { nodes, connections }
    }

    /// All connections, in the order the scene made them.
    #[must_use]
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// The connection feeding `input` on `handle`, if any.
    #[must_use]
    pub fn connection_into(
        &self,
        handle: &str,
        input: &str,
    ) -> Option<&Connection> {
        self.connections.iter().find(|connection| {
            connection.to_handle == handle && connection.to_input == input
        })
    }

    /// Returns whether anything is connected to `output` on `handle`.
    #[must_use]
    pub fn is_output_connected(&self, handle: &str, output: &str) -> bool {
        self.connections.iter().any(|connection| {
            connection.from_handle == handle && connection.from_output == output
        })
    }

    /// The node with this handle.
    #[must_use]
    pub fn node(&self, handle: &str) -> Option<&ShaderNode> {
        self.nodes.iter().find(|node| node.handle == handle)
    }

    /// The index of the node with this handle.
    #[must_use]
    pub fn node_index(&self, handle: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.handle == handle)
    }

    /// All `shader` nodes, in declaration order.
    #[must_use]
    pub fn nodes(&self) -> &[ShaderNode] {
        &self.nodes
    }

    /// Node indices in a deterministic topological order.
    ///
    /// Ties are broken by declaration order, so the same network always
    /// yields the same order -- which is what makes the parameter-block
    /// layout and the assembled module deterministic (requirement R6).
    /// Connections naming handles that do not exist are ignored here; the
    /// validator reports them.
    ///
    /// # Errors
    ///
    /// [`Error::Cycle`] if the connections form a cycle, naming the
    /// first node still blocked when no progress can be made.
    ///
    /// # Panics
    ///
    /// Never: the internal invariant is that an incomplete order leaves at
    /// least one node unemitted.
    pub fn topological_order(&self) -> Result<Vec<usize>, Error> {
        let mut emitted = vec![false; self.nodes.len()];
        let mut order = Vec::with_capacity(self.nodes.len());

        let dependencies: Vec<Vec<usize>> = self
            .nodes
            .iter()
            .map(|node| {
                self.connections
                    .iter()
                    .filter(|connection| connection.to_handle == node.handle)
                    .filter_map(|connection| {
                        self.node_index(&connection.from_handle)
                    })
                    .collect()
            })
            .collect();

        let mut cycle = None;

        while order.len() < self.nodes.len() && cycle.is_none() {
            let ready = (0..self.nodes.len()).find(|&index| {
                !emitted[index]
                    && dependencies[index]
                        .iter()
                        .all(|&dependency| emitted[dependency])
            });

            match ready {
                Some(index) => {
                    emitted[index] = true;
                    order.push(index);
                }
                None => {
                    let blocked = emitted
                        .iter()
                        .position(|done| !done)
                        .expect("a node remains when the order is incomplete");

                    cycle = Some(self.nodes[blocked].handle.clone());
                }
            }
        }

        cycle.map_or(Ok(order), |handle| Err(Error::Cycle { handle }))
    }
}
