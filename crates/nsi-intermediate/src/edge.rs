//! Connection classification.
//!
//! An ɴsɪ connection is a typed multi-relation: its meaning depends on
//! the destination attribute. Only [`EdgeKind::SurfaceShader`] and
//! [`EdgeKind::ShaderNetwork`] become object references in a target
//! renderer; the rest are scene membership, transform composition,
//! instancing, or output routing.
//!
//! Defaulting an unrecognised destination to a reference would be the
//! silent failure this module exists to prevent: a misclassified
//! connection does not fail loudly, it renders, with materials on the
//! wrong shapes or output routed nowhere.
//!
//! ɴsɪ's set of destinations is **open** -- its own §4.8 connects one
//! `attributes` node to another's `visibility` to override a value --
//! so it cannot be enumerated. Every `<connection>` attribute the
//! specification declares has a name here, pinned by
//! `tests/classifier.rs`; anything else becomes [`EdgeKind::Other`],
//! carrying its own name.
//!
//! Most classes are *carried*, not resolved: a backend reads them off
//! the edge list. Only membership, attribute binding, the shader slots,
//! instancing and the output chain are walked, so an unrecognised
//! destination cannot become a material by accident. The cost is that a
//! typo is now quiet rather than loud -- as it is in the renderer.

use crate::{OwnedArg, OwnedData};

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
    /// `shader -> camera "lensshader"`. ɴsɪ: "a lens shader is an osl
    /// network connected to a camera through the lensshader
    /// connection".
    LensShader,
    /// `outputlayer -> outputlayer "backgroundlayer"`.
    BackgroundLayer,
    /// `geometry -> attributes "bounds"`.
    Bounds,
    /// `set -> attributes "visibility.set.subsurface"`.
    SubsurfaceSet,
    /// `set -> .global "exclusiveshading"`.
    ExclusiveShading,
    /// `set -> geometry "facesets"`. ɴsɪ's own Listing 3.2.
    FaceSet,
    /// A connection to some other attribute, carried but never
    /// interpreted.
    ///
    /// ɴsɪ permits connecting a node to an arbitrary attribute -- the
    /// specification's own §4.8 connects one `attributes` node to
    /// another's `visibility` with a `"value"` argument -- so the set of
    /// legal destinations is open and cannot be enumerated. Rejecting
    /// what is not listed made legal scenes unrecordable; interpreting
    /// it would be the guess this module exists to refuse. So it is
    /// kept, with its name, and resolution ignores it.
    Other {
        /// The destination attribute, verbatim.
        to_attr: String,
    },
    /// `geo -> instances "sourcemodels"`.
    InstanceSource,
    /// `node -> set "members"`. A `set` node groups nodes so one
    /// connection stands for many.
    SetMember,
    /// `set -> outputlayer "lightset"`. ɴsɪ's documented light-set
    /// workflow connects lights to a `set`, then the set here.
    LightSet,
    /// `shaderattributes -> geometry|transform "shaderattributes"`.
    ShaderAttributes,
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
#[non_exhaustive]
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

impl EdgeKind {
    /// The ɴsɪ destination attribute this class came from.
    ///
    /// Inverse of [`classify`] for every class but
    /// [`EdgeKind::ShaderNetwork`], whose destination is a port name the
    /// caller already holds. The two must change together.
    pub fn to_attr(&self) -> &str {
        match self {
            Self::SceneMember => "objects",
            Self::AttributeBinding => "geometryattributes",
            Self::SurfaceShader => "surfaceshader",
            Self::DisplacementShader => "displacementshader",
            Self::VolumeShader => "volumeshader",
            Self::LensShader => "lensshader",
            Self::BackgroundLayer => "backgroundlayer",
            Self::Bounds => "bounds",
            Self::SubsurfaceSet => "visibility.set.subsurface",
            Self::ExclusiveShading => "exclusiveshading",
            Self::FaceSet => "facesets",
            Self::Other { to_attr } => to_attr,
            Self::InstanceSource => "sourcemodels",
            Self::SetMember => "members",
            Self::LightSet => "lightset",
            Self::ShaderAttributes => "shaderattributes",
            Self::Screen => "screens",
            Self::OutputLayer => "outputlayers",
            Self::OutputDriver => "outputdrivers",
            Self::ShaderNetwork { to_port, .. } => to_port,
        }
    }
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
        self.integer_argument("priority")
    }

    /// ɴsɪ's `"index"` connection argument, or `0` when absent.
    ///
    /// ɴsɪ: connections to `sourcemodels` "must have an integer index
    /// attribute if there are several, so the models effectively form an
    /// ordered list", and an `instances` node's `modelindices` "is
    /// matched to the index attribute of the model connection".
    pub fn index(&self) -> i32 {
        self.integer_argument("index")
    }

    /// ɴsɪ's `"strength"` connection argument, or `0` when absent.
    ///
    /// ɴsɪ: "a connection with a strength greater than 0 will block the
    /// progression of a recursive `NSIDelete`."
    pub fn strength(&self) -> i32 {
        self.integer_argument("strength")
    }

    /// One integer connection argument, or `0` when absent.
    fn integer_argument(&self, name: &str) -> i32 {
        self.args
            .iter()
            .find(|arg| arg.name == name)
            .and_then(|arg| match &arg.data {
                OwnedData::I32(values) => values.first().copied(),
                OwnedData::I64(values) => values.first().map(|v| *v as i32),
                _ => None,
            })
            .unwrap_or(0)
    }
}

/// Classify a connection by its destination attribute.
///
/// A *named* `from_attr` means the source names an output port, which
/// only happens for shader-network edges. `Some("")` is not a name:
/// ɴsɪ documents it as equivalent to `None`, meaning the `from` node
/// itself is connected, so it classifies by destination like any other
/// node-level connection.
pub fn classify(from_attr: Option<&str>, to_attr: &str) -> EdgeKind {
    // A named source port is always a shader network edge, whatever the
    // destination is called.
    if let Some(from_port) = from_attr.filter(|port| !port.is_empty()) {
        return EdgeKind::ShaderNetwork {
            from_port: from_port.to_string(),
            to_port: to_attr.to_string(),
        };
    }

    match to_attr {
        "objects" => EdgeKind::SceneMember,
        "geometryattributes" => EdgeKind::AttributeBinding,
        "surfaceshader" => EdgeKind::SurfaceShader,
        "displacementshader" => EdgeKind::DisplacementShader,
        "volumeshader" => EdgeKind::VolumeShader,
        "lensshader" => EdgeKind::LensShader,
        "backgroundlayer" => EdgeKind::BackgroundLayer,
        "bounds" => EdgeKind::Bounds,
        "visibility.set.subsurface" => EdgeKind::SubsurfaceSet,
        "exclusiveshading" => EdgeKind::ExclusiveShading,
        "sourcemodels" => EdgeKind::InstanceSource,
        "members" => EdgeKind::SetMember,
        "lightset" => EdgeKind::LightSet,
        "shaderattributes" => EdgeKind::ShaderAttributes,
        "screens" => EdgeKind::Screen,
        "outputlayers" => EdgeKind::OutputLayer,
        "outputdrivers" => EdgeKind::OutputDriver,
        "facesets" => EdgeKind::FaceSet,
        // Not an error: ɴsɪ's destinations are an open set. Carried,
        // never resolved.
        other => EdgeKind::Other {
            to_attr: other.to_string(),
        },
    }
}
