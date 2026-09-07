//! Typed handles, so a nonsense connection is a compile error.
//!
//! ɴsɪ addresses everything by string handle and connects everything
//! through string slots, so nothing stops you writing
//! `ctx.connect("camera", None, "screen", "outputlayers", None)` --
//! backwards, into a slot that does not accept a camera. The renderer
//! reports it, if you are lucky, at render time.
//!
//! A [`Handle<K>`] carries the node's kind in the type system. The
//! connection methods are then implemented only where ɴsɪ allows the
//! connection, so the same mistake does not compile.
//!
//! # The rules, and where they come from
//!
//! Every rule below is quoted from `nsi.pdf`, not inferred:
//!
//! | Slot | On | Accepts |
//! |------|----|---------|
//! | `objects` | `.root`, `transform` | *"geometry nodes, other transform nodes and camera nodes"* |
//! | `geometryattributes` | `transform` | *"attributes nodes"* |
//! | `shaderattributes` | `transform` | *"attributes nodes"* |
//! | `surfaceshader`, `displacementshader`, `volumeshader` | `attributes` | *"the shader node"* |
//! | `screens` | `camera` | *"screen nodes"* |
//! | `outputlayers` | `screen` | *"output layer nodes"* |
//! | `outputdrivers` | `outputlayer` | *"output driver nodes"* |
//!
//! `.root` is in the `objects` row because the spec calls it *"much
//! like a transform node"*, and its own example connects geometry
//! straight into `.root`'s `objects`.
//!
//! # Example
//!
//! ```no_run
//! # use nsi_ffi_wrap as nsi;
//! # use nsi_toolbelt::scene;
//! # let ctx = nsi::Context::new(None).unwrap();
//! let camera = scene::perspective_camera(&ctx, Some("camera"));
//! let screen = scene::screen(&ctx, Some("screen"));
//! let layer = scene::output_layer(&ctx, Some("beauty"));
//! let driver = scene::output_driver(&ctx, Some("driver"));
//!
//! scene::root().append(&ctx, &camera);
//! camera.screens(&ctx, &screen);
//! screen.output_layers(&ctx, &layer);
//! layer.output_drivers(&ctx, &driver);
//! ```
//!
//! Reverse any of those and it does not build. The error codes are
//! pinned, because a `compile_fail` test passes for *any* error --
//! including a typo in the test itself.
//!
//! A camera is not an output layer, so the argument does not typecheck:
//!
//! ```compile_fail,E0308
//! # use nsi_ffi_wrap as nsi;
//! # use nsi_toolbelt::scene;
//! # let ctx = nsi::Context::new(None).unwrap();
//! let camera = scene::perspective_camera(&ctx, None);
//! let screen = scene::screen(&ctx, None);
//! screen.output_layers(&ctx, &camera);
//! ```
//!
//! And a `mesh` has no `objects` slot at all, so the method is not
//! there to call:
//!
//! ```compile_fail,E0599
//! # use nsi_ffi_wrap as nsi;
//! # use nsi_toolbelt::scene;
//! # let ctx = nsi::Context::new(None).unwrap();
//! let outer = scene::mesh(&ctx, None);
//! let inner = scene::mesh(&ctx, None);
//! outer.append(&ctx, &inner);
//! ```

use crate::{generate_or_use_handle, transform};
use core::marker::PhantomData;
use nsi_ffi_wrap as nsi;

/// The node kinds the connection rules distinguish.
///
/// Zero-sized markers; they exist to be a type parameter, never a
/// value. Kinds are as coarse as the rules are -- ɴsɪ's `objects` slot
/// makes no distinction between a `mesh` and a `curves`, so neither
/// does [`Geometry`](kind::Geometry).
pub mod kind {
    /// `.root`, the end connection for everything renderable.
    pub struct Root;
    /// A `transform` node.
    pub struct Transform;
    /// Any renderable primitive: `mesh`, `curves`, `particles`, and so on.
    pub struct Geometry;
    /// Any of the camera nodes.
    pub struct Camera;
    /// An `attributes` node.
    pub struct Attributes;
    /// A `shader` node.
    pub struct Shader;
    /// A `screen` node.
    pub struct Screen;
    /// An `outputlayer` node.
    pub struct OutputLayer;
    /// An `outputdriver` node.
    pub struct OutputDriver;
}

/// A node handle that remembers what kind of node it names.
///
/// Deref-free on purpose: the point is that a `Handle<Camera>` is not
/// interchangeable with a `&str`, because that is exactly the
/// interchangeability that lets a wrong connection through.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle<K> {
    name: String,
    kind: PhantomData<fn() -> K>,
}

impl<K> Handle<K> {
    fn new(name: String) -> Self {
        Self {
            name,
            kind: PhantomData,
        }
    }

    /// The handle ɴsɪ knows this node by.
    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// Sets attributes on this node.
    ///
    /// Returns `&self`, so calls chain.
    pub fn set<'a>(
        &self,
        ctx: &nsi::Context<'a>,
        args: &nsi::ArgSlice<'_, 'a>,
    ) -> &Self {
        ctx.set_attribute(self.as_str(), args);
        self
    }
}

/// Nodes with an `objects` slot: `.root` and `transform`.
pub trait HasObjects {}
impl HasObjects for kind::Root {}
impl HasObjects for kind::Transform {}

/// Nodes an `objects` slot accepts: *"geometry nodes, other transform
/// nodes and camera nodes"*.
pub trait IsObject {}
impl IsObject for kind::Transform {}
impl IsObject for kind::Geometry {}
impl IsObject for kind::Camera {}

impl<K: HasObjects> Handle<K> {
    /// Connects `object` into this node's `objects` slot.
    ///
    /// Returns `&self` rather than the child, so a chain reads down the
    /// graph the way the scene does.
    pub fn append<O: IsObject>(
        &self,
        ctx: &nsi::Context,
        object: &Handle<O>,
    ) -> &Self {
        ctx.connect(object.as_str(), None, self.as_str(), "objects", None);
        self
    }
}

impl Handle<kind::Transform> {
    /// Connects an `attributes` node into `geometryattributes`.
    pub fn geometry_attributes(
        &self,
        ctx: &nsi::Context,
        attributes: &Handle<kind::Attributes>,
    ) -> &Self {
        ctx.connect(
            attributes.as_str(),
            None,
            self.as_str(),
            "geometryattributes",
            None,
        );
        self
    }

    /// Connects an `attributes` node into `shaderattributes`.
    pub fn shader_attributes(
        &self,
        ctx: &nsi::Context,
        attributes: &Handle<kind::Attributes>,
    ) -> &Self {
        ctx.connect(
            attributes.as_str(),
            None,
            self.as_str(),
            "shaderattributes",
            None,
        );
        self
    }
}

impl Handle<kind::Attributes> {
    /// Connects the shader that shades the surface.
    pub fn surface_shader(
        &self,
        ctx: &nsi::Context,
        shader: &Handle<kind::Shader>,
    ) -> &Self {
        self.shader_slot(ctx, shader, "surfaceshader")
    }

    /// Connects the shader that displaces the surface.
    pub fn displacement_shader(
        &self,
        ctx: &nsi::Context,
        shader: &Handle<kind::Shader>,
    ) -> &Self {
        self.shader_slot(ctx, shader, "displacementshader")
    }

    /// Connects the shader that shades the volume.
    pub fn volume_shader(
        &self,
        ctx: &nsi::Context,
        shader: &Handle<kind::Shader>,
    ) -> &Self {
        self.shader_slot(ctx, shader, "volumeshader")
    }

    fn shader_slot(
        &self,
        ctx: &nsi::Context,
        shader: &Handle<kind::Shader>,
        slot: &str,
    ) -> &Self {
        ctx.connect(shader.as_str(), None, self.as_str(), slot, None);
        self
    }
}

impl Handle<kind::Camera> {
    /// Connects a `screen` node into `screens`.
    pub fn screens(
        &self,
        ctx: &nsi::Context,
        screen: &Handle<kind::Screen>,
    ) -> &Self {
        ctx.connect(screen.as_str(), None, self.as_str(), "screens", None);
        self
    }
}

impl Handle<kind::Screen> {
    /// Connects an `outputlayer` node into `outputlayers`.
    pub fn output_layers(
        &self,
        ctx: &nsi::Context,
        layer: &Handle<kind::OutputLayer>,
    ) -> &Self {
        ctx.connect(layer.as_str(), None, self.as_str(), "outputlayers", None);
        self
    }
}

impl Handle<kind::OutputLayer> {
    /// Connects an `outputdriver` node into `outputdrivers`.
    pub fn output_drivers(
        &self,
        ctx: &nsi::Context,
        driver: &Handle<kind::OutputDriver>,
    ) -> &Self {
        ctx.connect(
            driver.as_str(),
            None,
            self.as_str(),
            "outputdrivers",
            None,
        );
        self
    }
}

/// `.root`. It is reserved and always exists, so nothing is created.
pub fn root() -> Handle<kind::Root> {
    Handle::new(nsi::node::ROOT.to_string())
}

/// Creates a node of `node_type` and returns a handle of kind `K`.
///
/// The escape hatch for node types this module does not name, and what
/// every constructor below is written in terms of.
///
/// # Correctness
/// `K` must be the kind ɴsɪ gives `node_type`; nothing checks it. Prefer
/// a named constructor where one exists.
pub fn node_of<K>(
    ctx: &nsi::Context,
    handle: Option<&str>,
    node_type: &str,
) -> Handle<K> {
    let handle = generate_or_use_handle(handle, Some(node_type));
    ctx.create(handle.as_str(), node_type, None);
    Handle::new(handle)
}

macro_rules! constructor {
    ($name:ident, $kind:ty, $node_type:expr, $doc:expr) => {
        #[doc = $doc]
        ///
        /// If `handle` is [`None`] a random one is generated.
        pub fn $name(
            ctx: &nsi::Context,
            handle: Option<&str>,
        ) -> Handle<$kind> {
            node_of(ctx, handle, $node_type)
        }
    };
}

constructor!(mesh, kind::Geometry, nsi::node::MESH, "Creates a `mesh`.");
constructor!(
    curves,
    kind::Geometry,
    nsi::node::CURVES,
    "Creates a `curves` node."
);
constructor!(
    particles,
    kind::Geometry,
    nsi::node::PARTICLES,
    "Creates a `particles` node."
);
constructor!(
    plane,
    kind::Geometry,
    nsi::node::PLANE,
    "Creates a `plane`."
);
constructor!(
    environment,
    kind::Geometry,
    nsi::node::ENVIRONMENT,
    "Creates an `environment` node."
);
constructor!(
    perspective_camera,
    kind::Camera,
    nsi::node::PERSPECTIVE_CAMERA,
    "Creates a `perspectivecamera`."
);
constructor!(
    orthographic_camera,
    kind::Camera,
    nsi::node::ORTHOGRAPHIC_CAMERA,
    "Creates an `orthographiccamera`."
);
constructor!(
    attributes,
    kind::Attributes,
    nsi::node::ATTRIBUTES,
    "Creates an `attributes` node."
);
constructor!(
    shader,
    kind::Shader,
    nsi::node::SHADER,
    "Creates a `shader` node."
);
constructor!(
    screen,
    kind::Screen,
    nsi::node::SCREEN,
    "Creates a `screen` node."
);
constructor!(
    output_layer,
    kind::OutputLayer,
    nsi::node::OUTPUT_LAYER,
    "Creates an `outputlayer` node."
);
constructor!(
    output_driver,
    kind::OutputDriver,
    nsi::node::OUTPUT_DRIVER,
    "Creates an `outputdriver` node."
);
constructor!(
    transform,
    kind::Transform,
    nsi::node::TRANSFORM,
    "Creates a bare `transform`, with no matrix set."
);

/// Creates a `transform` holding `matrix`.
///
/// The typed counterpart to the free functions in
/// [`transform`](crate::transform); use those directly when you want the
/// matrix without a node.
pub fn transform_with(
    ctx: &nsi::Context,
    handle: Option<&str>,
    matrix: &transform::Matrix,
) -> Handle<kind::Transform> {
    let handle: Handle<kind::Transform> =
        node_of(ctx, handle, nsi::node::TRANSFORM);
    ctx.set_attribute(
        handle.as_str(),
        &[nsi::matrix_f64!("transformationmatrix", matrix)],
    );
    handle
}

/// A `transform` that scales.
pub fn scaling(
    ctx: &nsi::Context,
    handle: Option<&str>,
    scale: &[f64; 3],
) -> Handle<kind::Transform> {
    transform_with(ctx, handle, &transform::scaling_matrix(scale))
}

/// A `transform` that translates.
pub fn translation(
    ctx: &nsi::Context,
    handle: Option<&str>,
    translate: &[f64; 3],
) -> Handle<kind::Transform> {
    transform_with(ctx, handle, &transform::translation_matrix(translate))
}

/// A `transform` that rotates `angle` **degrees** about `axis`.
pub fn rotation(
    ctx: &nsi::Context,
    handle: Option<&str>,
    angle: f64,
    axis: &[f64; 3],
) -> Handle<kind::Transform> {
    transform_with(ctx, handle, &transform::rotation_matrix(angle, axis))
}

/// A `transform` placing a camera at `eye`, looking at `to`.
pub fn look_at(
    ctx: &nsi::Context,
    handle: Option<&str>,
    eye: &[f64; 3],
    to: &[f64; 3],
    up: &[f64; 3],
) -> Handle<kind::Transform> {
    transform_with(ctx, handle, &transform::look_at_matrix(eye, to, up))
}
