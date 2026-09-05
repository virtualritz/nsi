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

use crate::OwnedArg;
use core::fmt;

/// What an ɴsɪ connection means, once classified.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
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
    /// `shader -> attributes "displacementshader"`.
    DisplacementShader,
    /// `shader -> attributes "volumeshader"`.
    VolumeShader,
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
    ShaderNetwork {
        /// The output port named on the source node.
        from_port: String,
        /// The input port named on the destination node.
        to_port: String,
    },
}

/// A recorded, classified connection.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// The handle the connection is made from.
    pub from: String,
    /// The handle the connection is made to.
    pub to: String,
    /// What the connection means.
    pub kind: EdgeKind,
    /// The arguments of the ɴsɪ `connect` call that made this edge, in
    /// call order.
    ///
    /// Kept whole rather than reduced to the one argument resolution
    /// needs, so `"strength"` -- which blocks a recursive delete -- and
    /// `"value"` survive for a backend that wants them, and so replay
    /// can emit what was passed.
    pub args: Vec<OwnedArg>,
}

impl Edge {
    /// ɴsɪ's `"priority"` connection argument, or `0` when absent.
    ///
    /// ɴsɪ ranks a repeated attribute definition by "the highest
    /// priority", so a larger number wins. See
    /// [`Scene::geometry_binding`].
    ///
    /// [`Scene::geometry_binding`]: crate::Scene::geometry_binding
    pub fn priority(&self) -> i32 {
        self.args
            .iter()
            .find(|arg| arg.name == "priority")
            .and_then(|arg| match &arg.data {
                crate::OwnedData::I32(values) => values.first().copied(),
                crate::OwnedData::I64(values) => {
                    values.first().map(|v| *v as i32)
                }
                _ => None,
            })
            .unwrap_or(0)
    }
}

/// An ɴsɪ connection whose destination attribute has no mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassifyError {
    /// The destination attribute that has no mapping.
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
        "displacementshader" => EdgeKind::DisplacementShader,
        "volumeshader" => EdgeKind::VolumeShader,
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
