//! Standard ɴsɪ node types.

/// Wildcard node that references all existing nodes at once (`.all`).
pub const ALL: &str = ".all";
/// The scene’s root (`.root`).
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-root).
pub const ROOT: &str = ".root";
/// Global settings node (`.global`).
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#the-global-node).
pub const GLOBAL: &str = ".global";
/// Expresses relationships of groups of nodes.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-set).
pub const SET: &str = "set";
/// [ᴏsʟ](http://opensource.imageworks.com/osl.html) shader or layer in a shader
/// group.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-shader).
pub const SHADER: &str = "shader";
/// Container for generic attributes (e.g. visibility).
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-attributes).
pub const ATTRIBUTES: &str = "attributes";
/// Transformation to place objects in the scene.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-transform).
pub const TRANSFORM: &str = "transform";
/// Specifies instances of other nodes.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-instances).
pub const INSTANCES: &str = "instances";
/// An infinite plane.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-plane).
pub const PLANE: &str = "plane";
/// Polygonal mesh or subdivision surface.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-mesh).
pub const MESH: &str = "mesh";
/// Assign attributes to part of a mesh, curves or particles.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-faceset).
pub const FACESET: &str = "faceset";
/// Linear, b-spline and Catmull-Rom curves.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-curves).
pub const CURVES: &str = "curves";
/// Collection of particles.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-particles).
pub const PARTICLES: &str = "particles";
/// NURBS surface with optional trim curves.
///
/// Intrinsic attributes: `nu`, `nv` (i32, control-point counts);
/// `uorder`, `vorder` (i32, ≥ 2); `uknot`, `vknot` (f32 array, lengths
/// `nu + uorder` and `nv + vorder`); and either `P` (point) or `Pw`
/// (rational, f32[4]). Optional trim is the `trimcurves.*` family
/// (`nloops`, `ncurves`, `n`, `order`, `knot`, `min`, `max`, `u`, `v`,
/// `w`, `sense`).
pub const NURBS: &str = "nurbs";
/// Geometry to be loaded or generated in delayed fashion.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-procedural).
pub const PROCEDURAL: &str = "procedural";
/// A volume loaded from an [OpenVDB](https://www.openvdb.org) file.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-volume).
///
/// Also see the `volume` example.
pub const VOLUME: &str = "volume";
/// Geometry type to define environment lighting.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-environment).
pub const ENVIRONMENT: &str = "environment";
/// An orthographic camera.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#the-orthographiccamera-node).
pub const ORTHOGRAPHIC_CAMERA: &str = "orthographiccamera";
/// A perspective camera.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#the-perspectivecamera-node).
pub const PERSPECTIVE_CAMERA: &str = "perspectivecamera";
/// A fisheye camera.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#the-fisheyecamera-node).
pub const FISHEYE_CAMERA: &str = "fisheyecamera";
/// A cylindrical camera.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#the-cylindricalcamera-node).
pub const CYLINDRICAL_CAMERA: &str = "cylindricalcamera";
/// A spherical camera.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#the-sphericalcamera-node).
pub const SPHERICAL_CAMERA: &str = "sphericalcamera";
/// A target where to output rendered pixels.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputdriver).
pub const OUTPUT_DRIVER: &str = "outputdriver";
/// Describes one render layer to be connected to an `outputdriver` node.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputlayer).
pub const OUTPUT_LAYER: &str = "outputlayer";
/// Describes how the view from a camera node will be rasterized into an
/// `outputlayer` node.
/// [🕮](https://nsi.readthedocs.io/en/latest/nodes.html#node-screen).
pub const SCREEN: &str = "screen";
