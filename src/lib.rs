//! A Flexible, Modern API for Renderers
//!
//! The [Nodal Scene Interface](https://nsi.readthedocs.io/) (ɴsɪ) is
//! built around the concept of nodes.
//! Each node has a unique handle to identify it and a type which
//! describes its intended function in the scene. Nodes are abstract
//! containers for data. The interpretation depends on the node type.
//! Nodes can also be [connected to each
//! other](https://nsi.readthedocs.io/en/latest/guidelines.html#basic-scene-anatomy)
//! to express relationships.
//!
//! Data is stored on nodes as attributes. Each attribute has a name which
//! is unique on the node and a type which describes the kind of data it
//! holds (strings, integer numbers, floating point numbers, etc).
//!
//! Relationships and data flow between nodes are represented as
//! connections. Connections have a source and a destination. Both can be
//! either a node or a specific attribute of a node. There are no type
//! restrictions for connections in the interface itself. It is acceptable
//! to connect attributes of different types or even attributes to nodes.
//! The validity of such connections depends on the types of the nodes
//! involved.
//!
//! What we refer to as the ɴsɪ has two major components:
//!
//! 1.  Methods to create nodes, attributes and their connections. These
//!     are attached to a rendering [`Context`],
//!
//! 2.  [Node types](https://nsi.readthedocs.io/en/latest/nodes.html)
//!     understood by the renderer.
//!
//! Much of the complexity and expressiveness of the interface comes from
//! the supported nodes.
//!
//! The first part was kept deliberately simple to make it easy to support
//! multiple ways of creating nodes.
#![allow(incomplete_features)]
#![feature(specialization)]
#![feature(const_generics)]

extern crate self as nsi;
use nsi_sys;
#[allow(unused_imports)]
use std::{
    ffi::{CStr, CString},
    ops::Drop,
    vec::Vec,
};

#[macro_use]
mod argument;
pub use argument::*;

mod test;

static STR_ERROR: &str = "Found null byte in the middle of the string";

//type Handle = impl Into<Vec<u8>>;

/// An ɴsɪ context.
///
/// Also see the [ɴsɪ docmentation on context
/// handling](https://nsi.readthedocs.io/en/latest/c-api.html#context-handling).
pub struct Context {
    context: nsi_sys::NSIContext_t,
}

impl Context {
    /// Creates an ɴsɪ context.
    ///
    /// Contexts may be used in multiple threads at once.
    ///
    /// If this method fails for some reason, it returns [`None`].
    /// ```
    /// // Create rendering context that dumps to stdout.
    /// let c = nsi::Context::new(&vec![nsi::Arg::new(
    ///     "streamfilename",
    ///     &String::from("stdout"),
    /// )]).expect("Could not create ɴsɪ context.");
    /// ```
    pub fn new(args: &ArgVec) -> Option<Self> {
        match {
            if args.is_empty() {
                unsafe { nsi_sys::NSIBegin(0, std::ptr::null()) }
            } else {
                let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
                get_c_param_vec!(args, &mut args_out);

                unsafe {
                    nsi_sys::NSIBegin(
                        args_out.len() as i32,
                        args_out.as_ptr() as *const nsi_sys::NSIParam_t,
                    )
                }
            }
        } {
            0 => None,
            ref c => Some(Self { context: *c }),
        }
    }

    /// This function is used to create a new node.
    ///
    /// # Arguments
    ///
    /// * `handle` - A node handle. This string will uniquely identify
    ///   the node in the scene.
    ///
    ///   If the supplied handle matches an existing node, the function
    ///   does nothing if all other parameters match the call which
    ///   created that node. Otherwise, it emits an error. Note that
    ///   handles need only be unique within a given interface context.
    ///   It is acceptable to reuse the same handle inside different
    ///   contexts.
    ///
    /// * `type` - The type of node to create.
    ///
    /// * `args` - A [`Vec`] of optional [`Arg`] arguments. *There are
    ///   no optional parameters defined as of now*.
    pub fn create(&self, handle: impl Into<Vec<u8>>, node_type: &Node, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSICreate(
                self.context,
                CString::new(handle.into()).expect(STR_ERROR).as_ptr(),
                node_type.as_c_str().as_ptr() as *const i8,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            )
        }
    }

    pub fn delete(&self, handle: impl Into<Vec<u8>>, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSIDelete(
                self.context,
                CString::new(handle.into()).expect(STR_ERROR).as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn set_attribute(&self, object: impl Into<Vec<u8>>, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSISetAttribute(
                self.context,
                CString::new(object.into()).expect(STR_ERROR).as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn set_attribute_at_time(&self, object: impl Into<Vec<u8>>, time: f64, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSISetAttributeAtTime(
                self.context,
                CString::new(object.into()).expect(STR_ERROR).as_ptr(),
                time,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn connect(
        &self,
        from: impl Into<Vec<u8>>,
        from_attr: impl Into<Vec<u8>>,
        to: impl Into<Vec<u8>>,
        to_attr: impl Into<Vec<u8>>,
        args: &ArgVec,
    ) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSIConnect(
                self.context,
                CString::new(from.into()).expect(STR_ERROR).as_ptr(),
                CString::new(from_attr.into()).expect(STR_ERROR).as_ptr(),
                CString::new(to.into()).expect(STR_ERROR).as_ptr(),
                CString::new(to_attr.into()).expect(STR_ERROR).as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn disconnect(
        &self,
        from: impl Into<Vec<u8>>,
        from_attr: impl Into<Vec<u8>>,
        to: impl Into<Vec<u8>>,
        to_attr: impl Into<Vec<u8>>,
    ) {
        unsafe {
            nsi_sys::NSIDisconnect(
                self.context,
                CString::new(from.into()).expect(STR_ERROR).as_ptr(),
                CString::new(from_attr.into()).expect(STR_ERROR).as_ptr(),
                CString::new(to.into()).expect(STR_ERROR).as_ptr(),
                CString::new(to_attr.into()).expect(STR_ERROR).as_ptr(),
            );
        }
    }

    pub fn evaluate(&self, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSIEvaluate(
                self.context,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn render_control(&self, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec!(args, &mut args_out);

        unsafe {
            nsi_sys::NSIRenderControl(
                self.context,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            nsi_sys::NSIEnd(self.context);
        }
    }
}

/// A node in the ɴsɪ scene graph.
pub enum Node {
    /// The scene’s root.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-root).
    Root, // = ".root",
    /// Global settings node.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#the-global-node).
    Global,
    /// Expresses relationships of groups of nodes.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-set).
    Set,
    /// [ᴏsʟ](http://opensource.imageworks.com/?p=osl) shader or layer in a shader group.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-shader).
    Shader,
    /// Container for generic attributes (e.g. visibility).
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-attributes).
    Attributes,
    /// Transformation to place objects in the scene.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-transform).
    Transform,
    /// Specifies instances of other nodes.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-instances).
    Instances,
    /// An infinite plane.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-plane).
    Plane,
    /// Polygonal mesh or subdivision surface.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-mesh).
    Mesh,
    /// Assign attributes to part of a mesh, curves or paticles.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-faceset).
    FaceSet,
    /// Linear, b-spline and Catmull-Rom curves.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-curves).
    Curves,
    /// Collection of particles.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-particles).
    Particles,
    /// Geometry to be loaded or generated in delayed fashion.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-procedural).
    Procedural,
    /// A volume loaded from an [OpenVDB](https://www.openvdb.org) file.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-volume).
    Volume,
    // Geometry type to define environment lighting.
    // [Documentation]((https://nsi.readthedocs.io/en/latest/nodes.html#node-environment).
    Environment,
    /// Set of nodes to create viewing cameras.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-camera).
    Camera,
    OrthographicCamera,
    PerspectiveCamera,
    FisheyeCamera,
    CylindricalCamera,
    SphericalCamera,
    /// A target where to output rendered pixels.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputdriver).
    OutputDriver,
    /// Describes one render layer to be connected to an `outputdriver` node.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputlayer).
    OutputLayer,
    /// Describes how the view from a camera node will be rasterized into an outputlayer node.
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-screen).
    Screen,
}

impl Node {
    fn as_c_str(&self) -> &'static [u8] {
        match *self {
            Node::Root => b".root\0",
            Node::Global => b".global\0",
            Node::Set => b"set\0",
            Node::Plane => b"plane\0",
            Node::Shader => b"shader\0",
            Node::Attributes => b"attributes\0",
            Node::Transform => b"transform\0",
            Node::Instances => b"instances\0",
            Node::Mesh => b"mesh\0",
            Node::FaceSet => b"faceset\0",
            Node::Curves => b"curves\0",
            Node::Particles => b"particles\0",
            Node::Procedural => b"procedural\0",
            Node::Volume => b"volume\0",
            Node::Environment => b"environment\0",
            Node::Camera => b"camera\0",
            Node::OrthographicCamera => b"orthographiccamera\0",
            Node::PerspectiveCamera => b"perspectivecamera\0",
            Node::FisheyeCamera => b"fisheyecamera\0",
            Node::CylindricalCamera => b"cylindricalcamera\0",
            Node::SphericalCamera => b"sphericalcamera\0",
            Node::OutputDriver => b"outputdriver\0",
            Node::OutputLayer => b"outputlayer\0",
            Node::Screen => b"screen\0",
        }
    }
}
