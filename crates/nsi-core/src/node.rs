//! Known node types in the ɴsɪ scene graph.

/// Wildcard node that references all existing nodes at once (`.all`).
pub const ALL: &'static str = ".all";
/// The scene’s root (`.root`).
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-root).
pub const ROOT: &'static str = ".root";
/// Global settings node (`.global`).
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-global-node).
pub const GLOBAL: &'static str = ".global";
/// Expresses relationships of groups of nodes.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-set).
pub const SET: &'static str = "set";
/// [ᴏsʟ](http://opensource.imageworks.com/osl.html) shader or layer in a shader group.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-shader).
pub const SHADER: &'static str = "shader";
/// Container for generic attributes (e.g. visibility).
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-attributes).
pub const ATTRIBUTES: &'static str = "attributes";
/// Transformation to place objects in the scene.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-transform).
pub const TRANSFORM: &'static str = "transform";
/// Specifies instances of other nodes.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-instances).
pub const INSTANCES: &'static str = "instances";
/// An infinite plane.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-plane).
pub const PLANE: &'static str = "plane";
/// Polygonal mesh or subdivision surface.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-mesh).
pub const MESH: &'static str = "mesh";
/// Assign attributes to part of a mesh, curves or particles.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-faceset).
pub const FACESET: &'static str = "faceset";
/// Linear, b-spline and Catmull-Rom curves.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-curves).
pub const CURVES: &'static str = "curves";
/// Collection of particles.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-particles).
pub const PARTICLES: &'static str = "particles";
/// Geometry to be loaded or generated in delayed fashion.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-procedural).
pub const PROCEDURAL: &'static str = "procedural";
/// A volume loaded from an [OpenVDB](https://www.openvdb.org) file.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-volume).
///
/// Also see the `volume` example.
pub const VOLUME: &'static str = "volume";
/// Geometry type to define environment lighting.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-environment).
pub const ENVIRONMENT: &'static str = "environment";
/// An orthographic camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-orthographiccamera-node).
pub const ORTHOGRAPHIC_CAMERA: &'static str = "orthographiccamera";
/// A perspective camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-perspectivecamera-node).
pub const PERSPECTIVE_CAMERA: &'static str = "perspectivecamera";
/// A fisheye camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-fisheyecamera-node).
pub const FISHEYE_CAMERA: &'static str = "fisheyecamera";
/// A cylindrical camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-cylindricalcamera-node).
pub const CYLINDRICAL_CAMERA: &'static str = "cylindricalcamera";
/// A spherical camera.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-sphericalcamera-node).
pub const SPHERICAL_CAMERA: &'static str = "sphericalcamera";
/// A target where to output rendered pixels.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputdriver).
pub const OUTPUT_DRIVER: &'static str = "outputdriver";
/// Describes one render layer to be connected to an `outputdriver` node.
/// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputlayer).
pub const OUTPUT_LAYER: &'static str = "outputlayer";
/// Describes how the view from a camera node will be rasterized into an
/// `outputlayer` node. [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-screen).
pub const SCREEN: &'static str = "screen";
