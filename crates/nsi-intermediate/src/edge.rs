//! Connection classification.
//!
//! An ɴsɪ connection is a typed multi-relation: its meaning depends on
//! the destination attribute. Only [`EdgeKind::SurfaceShader`] and
//! [`EdgeKind::ShaderNetwork`] become object references in a target
//! renderer; the rest are scene membership, transform composition,
//! instancing, or output routing.
//!
//! Unrecognised destinations are rejected. Defaulting them to a
//! reference is exactly the silent failure this module exists to
//! prevent: a misclassified connection does not fail loudly, it renders,
//! with materials on the wrong shapes or output routed nowhere.

use core::fmt;

/// What an ɴsɪ connection means, once classified.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// `X -> .root "objects"`, or a transform chain link. Membership and
    /// hierarchy share one ɴsɪ attribute; which one an edge is depends
    /// on whether the destination is `.root`, so the recorder resolves
    /// that when it walks the graph rather than here.
    SceneMember,
    /// `attributes -> geo "geometryattributes"`. The attributes node has
    /// no counterpart in either target renderer and is dissolved at
    /// flush time.
    AttributeBinding,
    /// `shader -> attributes "surfaceshader"`. Becomes the shape's
    /// material.
    SurfaceShader,
    /// `geo -> instances "sourcemodels"`.
    InstanceSource,
    /// `screen -> camera "screens"`.
    Screen,
    /// `outputlayer -> screen "outputlayers"`.
    OutputLayer,
    /// `outputdriver -> outputlayer "outputdrivers"`.
    OutputDriver,
    /// An attribute-to-attribute shader network edge, naming ports on
    /// both ends.
    ///
    /// This maps 1:1 onto OSL's `ConnectShaders`, which is unsurprising:
    /// ɴsɪ was designed around OSL. Renderers whose references point at
    /// whole objects rather than attributes need an adapter here.
    ShaderNetwork { from_port: String, to_port: String },
}

/// A recorded, classified connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    /// The `"priority"` argument of the ɴsɪ `connect` call that made
    /// this edge, defaulting to `0`.
    ///
    /// ɴsɪ documents it as deciding "in which order the nodes should be
    /// considered when evaluating the value of an attribute", which is
    /// why [`Scene::geometry_binding`] reads it. Only
    /// [`EdgeKind::AttributeBinding`] edges consult it today.
    ///
    /// [`Scene::geometry_binding`]: crate::Scene::geometry_binding
    pub priority: i32,
}

/// An ɴsɪ connection whose destination attribute has no mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassifyError {
    pub to_attr: String,
}

impl fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unmapped ɴsɪ connection destination attribute {:?}; refusing \
             to guess -- add a case to nsi_intermediate::classify",
            self.to_attr
        )
    }
}

impl core::error::Error for ClassifyError {}

/// Classify a connection by its destination attribute.
///
/// A *named* `from_attr` means the source names an output port, which
/// only happens for shader-network edges. `Some("")` is not a name:
/// ɴsɪ documents it as equivalent to `None`, meaning the `from` node
/// itself is connected, so it classifies by destination like any other
/// node-level connection.
pub fn classify(
    from_attr: Option<&str>,
    to_attr: &str,
) -> Result<EdgeKind, ClassifyError> {
    // A named source port is always a shader network edge, whatever the
    // destination is called.
    if let Some(from_port) = from_attr.filter(|port| !port.is_empty()) {
        return Ok(EdgeKind::ShaderNetwork {
            from_port: from_port.to_string(),
            to_port: to_attr.to_string(),
        });
    }

    Ok(match to_attr {
        "objects" => EdgeKind::SceneMember,
        "geometryattributes" => EdgeKind::AttributeBinding,
        "surfaceshader" => EdgeKind::SurfaceShader,
        "sourcemodels" => EdgeKind::InstanceSource,
        "screens" => EdgeKind::Screen,
        "outputlayers" => EdgeKind::OutputLayer,
        "outputdrivers" => EdgeKind::OutputDriver,
        other => {
            return Err(ClassifyError {
                to_attr: other.to_string(),
            });
        }
    })
}
