//! Standard ɴsɪ node types and constants.

/// Wildcard node that references all existing nodes at once (`.all`).
pub const ALL: &str = ".all";

/// The scene's root (`.root`).
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-root).
pub const ROOT: &str = ".root";

/// Global settings node (`.global`).
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-global-node).
pub const GLOBAL: &str = ".global";

/// Expresses relationships of groups of nodes.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-set).
pub const SET: &str = "set";

/// [OSL](http://opensource.imageworks.com/osl.html) shader or layer in a shader group.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-shader).
pub const SHADER: &str = "shader";

/// Container for generic attributes (e.g. visibility).
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-attributes).
pub const ATTRIBUTES: &str = "attributes";

/// Transformation to place objects in the scene.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-transform).
pub const TRANSFORM: &str = "transform";

/// Specifies instances of other nodes.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-instances).
pub const INSTANCES: &str = "instances";

/// An infinite plane.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-plane).
pub const PLANE: &str = "plane";

/// Polygonal mesh or subdivision surface.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-mesh).
pub const MESH: &str = "mesh";

/// Assign attributes to part of a mesh, curves or particles.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-faceset).
pub const FACESET: &str = "faceset";

/// Linear, b-spline and Catmull-Rom curves.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-curves).
pub const CURVES: &str = "curves";

/// Collection of particles.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-particles).
pub const PARTICLES: &str = "particles";

/// Geometry to be loaded or generated in delayed fashion.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-procedural).
pub const PROCEDURAL: &str = "procedural";

/// A volume loaded from an [OpenVDB](https://www.openvdb.org) file.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-volume).
pub const VOLUME: &str = "volume";

/// Geometry type to define environment lighting.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-environment).
pub const ENVIRONMENT: &str = "environment";

/// An orthographic camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-orthographiccamera-node).
pub const ORTHOGRAPHIC_CAMERA: &str = "orthographiccamera";

/// A perspective camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-perspectivecamera-node).
pub const PERSPECTIVE_CAMERA: &str = "perspectivecamera";

/// A fisheye camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-fisheyecamera-node).
pub const FISHEYE_CAMERA: &str = "fisheyecamera";

/// A cylindrical camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-cylindricalcamera-node).
pub const CYLINDRICAL_CAMERA: &str = "cylindricalcamera";

/// A spherical camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-sphericalcamera-node).
pub const SPHERICAL_CAMERA: &str = "sphericalcamera";

/// A target where to output rendered pixels.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputdriver).
pub const OUTPUT_DRIVER: &str = "outputdriver";

/// Describes one render layer to be connected to an `outputdriver` node.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputlayer).
pub const OUTPUT_LAYER: &str = "outputlayer";

/// Describes how the view from a camera node will be rasterized into an
/// `outputlayer` node.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-screen).
pub const SCREEN: &str = "screen";

/// Node types in the ɴsɪ scene graph.
///
/// Each variant corresponds to a specific node type that can be created
/// in the scene graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NodeType {
    /// Wildcard that references all existing nodes at once.
    All,
    /// The scene's root node.
    Root,
    /// Global settings node.
    Global,
    /// Expresses relationships of groups of nodes.
    Set,
    /// OSL shader or layer in a shader group.
    Shader,
    /// Container for generic attributes (e.g. visibility).
    Attributes,
    /// Transformation to place objects in the scene.
    Transform,
    /// Specifies instances of other nodes.
    Instances,
    /// An infinite plane.
    Plane,
    /// Polygonal mesh or subdivision surface.
    Mesh,
    /// Assign attributes to part of a mesh, curves or particles.
    FaceSet,
    /// Linear, b-spline and Catmull-Rom curves.
    Curves,
    /// Collection of particles.
    Particles,
    /// Geometry to be loaded or generated in delayed fashion.
    Procedural,
    /// A volume loaded from an OpenVDB file.
    Volume,
    /// Geometry type to define environment lighting.
    Environment,
    /// An orthographic camera.
    OrthographicCamera,
    /// A perspective camera.
    PerspectiveCamera,
    /// A fisheye camera.
    FisheyeCamera,
    /// A cylindrical camera.
    CylindricalCamera,
    /// A spherical camera.
    SphericalCamera,
    /// A target where to output rendered pixels.
    OutputDriver,
    /// Describes one render layer to be connected to an output driver.
    OutputLayer,
    /// Describes how the view from a camera will be rasterized.
    Screen,
}

impl NodeType {
    /// Returns the string identifier used by the C API.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::All => ALL,
            Self::Root => ROOT,
            Self::Global => GLOBAL,
            Self::Set => SET,
            Self::Shader => SHADER,
            Self::Attributes => ATTRIBUTES,
            Self::Transform => TRANSFORM,
            Self::Instances => INSTANCES,
            Self::Plane => PLANE,
            Self::Mesh => MESH,
            Self::FaceSet => FACESET,
            Self::Curves => CURVES,
            Self::Particles => PARTICLES,
            Self::Procedural => PROCEDURAL,
            Self::Volume => VOLUME,
            Self::Environment => ENVIRONMENT,
            Self::OrthographicCamera => ORTHOGRAPHIC_CAMERA,
            Self::PerspectiveCamera => PERSPECTIVE_CAMERA,
            Self::FisheyeCamera => FISHEYE_CAMERA,
            Self::CylindricalCamera => CYLINDRICAL_CAMERA,
            Self::SphericalCamera => SPHERICAL_CAMERA,
            Self::OutputDriver => OUTPUT_DRIVER,
            Self::OutputLayer => OUTPUT_LAYER,
            Self::Screen => SCREEN,
        }
    }

    /// Parse a node type from its string identifier.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            ALL => Self::All,
            ROOT => Self::Root,
            GLOBAL => Self::Global,
            SET => Self::Set,
            SHADER => Self::Shader,
            ATTRIBUTES => Self::Attributes,
            TRANSFORM => Self::Transform,
            INSTANCES => Self::Instances,
            PLANE => Self::Plane,
            MESH => Self::Mesh,
            FACESET => Self::FaceSet,
            CURVES => Self::Curves,
            PARTICLES => Self::Particles,
            PROCEDURAL => Self::Procedural,
            VOLUME => Self::Volume,
            ENVIRONMENT => Self::Environment,
            ORTHOGRAPHIC_CAMERA => Self::OrthographicCamera,
            PERSPECTIVE_CAMERA => Self::PerspectiveCamera,
            FISHEYE_CAMERA => Self::FisheyeCamera,
            CYLINDRICAL_CAMERA => Self::CylindricalCamera,
            SPHERICAL_CAMERA => Self::SphericalCamera,
            OUTPUT_DRIVER => Self::OutputDriver,
            OUTPUT_LAYER => Self::OutputLayer,
            SCREEN => Self::Screen,
            _ => return None,
        })
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_type_round_trip() {
        let types = [
            NodeType::All,
            NodeType::Root,
            NodeType::Global,
            NodeType::Set,
            NodeType::Shader,
            NodeType::Attributes,
            NodeType::Transform,
            NodeType::Instances,
            NodeType::Plane,
            NodeType::Mesh,
            NodeType::FaceSet,
            NodeType::Curves,
            NodeType::Particles,
            NodeType::Procedural,
            NodeType::Volume,
            NodeType::Environment,
            NodeType::OrthographicCamera,
            NodeType::PerspectiveCamera,
            NodeType::FisheyeCamera,
            NodeType::CylindricalCamera,
            NodeType::SphericalCamera,
            NodeType::OutputDriver,
            NodeType::OutputLayer,
            NodeType::Screen,
        ];

        for ty in types {
            let s = ty.as_str();
            let parsed = NodeType::from_name(s).expect("should parse");
            assert_eq!(ty, parsed);
        }
    }
}
