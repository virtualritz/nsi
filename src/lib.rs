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
//! Data is stored on nodes as attributes. Each attribute has a name
//! which is unique on the node and a type which describes the kind of
//! data it holds (strings, integer numbers, floating point numbers,
//! etc).
//!
//! Relationships and data flow between nodes are represented as
//! connections. Connections have a source and a destination. Both can
//! be either a node or a specific attribute of a node. There are no
//! type restrictions for connections in the interface itself. It is
//! acceptable to connect attributes of different types or even
//! attributes to nodes. The validity of such connections depends on the
//! types of the nodes involved.
//!
//! What we refer to as the ɴsɪ has two major components:
//!
//! 1.  Methods to create nodes, attributes and their connections. These
//!     are attached to a rendering [`Context`],
//!
//! 2.  [Node types](https://nsi.readthedocs.io/en/latest/nodes.html)
//!     understood by the renderer.
//!
//! Much of the complexity and expressiveness of the interface comes
//! from the supported nodes.
//!
//! The first part was kept deliberately simple to make it easy to
//! support multiple ways of creating nodes.

extern crate self as nsi;
use nsi_sys;
#[allow(unused_imports)]
use std::{ffi::CString, ops::Drop, vec::Vec};

#[macro_use]
mod argument;
pub use argument::*;

mod tests;

//type Handle = impl Into<Vec<u8>>;

/// An ɴsɪ context.
///
/// Also see the [ɴsɪ docmentation on context
/// handling](https://nsi.readthedocs.io/en/latest/c-api.html#context-handling).
pub struct Context {
    context: nsi_sys::NSIContext_t,
}

impl From<nsi_sys::NSIContext_t> for Context {
    #[allow(dead_code)]
    #[inline]
    fn from(context: nsi_sys::NSIContext_t) -> Self {
        Self { context }
    }
}

impl Context {
    /// Creates an ɴsɪ context.
    ///
    /// Contexts may be used in multiple threads at once.
    ///
    /// # Example
    /// ```
    /// // Create rendering context that dumps to stdout.
    /// let c = nsi::Context::new(&[nsi::string!(
    ///     "streamfilename",
    ///     "stdout"
    /// )]).expect("Could not create ɴsɪ context.");
    /// ```
    /// # Error
    /// If this method fails for some reason, it returns [`None`].
    #[inline]
    pub fn new(args: &arg::ArgSlice) -> Option<Self> {
        match {
            if args.is_empty() {
                unsafe { nsi_sys::NSIBegin(0, std::ptr::null()) }
            } else {
                let args_out = arg::get_c_param_vec(args);

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
    /// * `node_type` – The type of node to create.
    ///
    /// * `args` – A [`std::slice`] of optional [`arg::Arg`] arguments. *There are
    ///   no optional parameters defined as of now*.
    #[inline]
    pub fn create(&self, handle: impl Into<Vec<u8>>, node_type: Node, args: &arg::ArgSlice) {
        let args_out = arg::get_c_param_vec(args);

        unsafe {
            nsi_sys::NSICreate(
                self.context,
                CString::new(handle).unwrap().as_ptr(),
                node_type.as_c_str().as_ptr() as *const i8,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            )
        }
    }

    /// This function deletes a node from the scene. All connections to
    /// and from the node are also deleted.
    ///
    /// Note that it is not possible to delete the `.root` or the
    /// `.global` nodes.
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///              [`Context::create()`].
    ///
    /// * `args` – A [`std::slice`] of optional [`arg::Arg`] arguments.
    ///
    /// # Optional Arguments
    ///
    /// * `"recursive"` ([`arg::ArgData::Integer`]) – Specifies whether
    ///   deletion is recursive. By default, only the specified node is
    ///   deleted. If a value of `1` is given, then nodes which connect
    ///   to the specified node are recursively removed. Unless they
    ///   meet one of the following conditions:
    ///   * They also have connections which do not eventually lead to the specified node.
    ///   * Their connection to the deleted node was created with a `strength` greater than `0`.
    ///
    ///   This allows, for example, deletion of an entire shader network in a single call.
    #[inline]
    pub fn delete(&self, handle: impl Into<Vec<u8>>, args: &arg::ArgSlice) {
        let args_out = arg::get_c_param_vec(args);

        unsafe {
            nsi_sys::NSIDelete(
                self.context,
                CString::new(handle).unwrap().as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    /// This functions sets attributes on a previously node.
    /// All optional arguments of the function become attributes of
    /// the node.
    ///
    /// On a [`Node::Shader`], this function is used to set the implicitly
    /// defined shader arguments.
    ///
    /// Setting an attribute using this function replaces any value
    ///  previously set by [`Context::set_attribute()`] or
    /// [`Context::set_attribute_at_time()`].
    /// To reset an attribute to its default value, use
    /// [`Context::delete_attribute()`]).
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///               [`Context::create()`].
    ///
    /// * `args` – A [`std::slice`] of optional [`arg::Arg`] arguments.
    #[inline]
    pub fn set_attribute(&self, handle: impl Into<Vec<u8>>, args: &arg::ArgSlice) {
        let args_out = arg::get_c_param_vec(args);

        unsafe {
            nsi_sys::NSISetAttribute(
                self.context,
                CString::new(handle).unwrap().as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    /// This function sets time-varying attributes (i.e. motion blurred).
    ///
    /// The `time` argument specifies at which time the attribute is being
    /// defined.
    ///
    /// It is not required to set time-varying attributes in any
    /// particular order. In most uses, attributes that are motion blurred must
    /// have the same specification throughout the time range.
    ///
    /// A notable  exception is the `P` attribute on (particles)[`Node::Particles`]
    /// which can be of different size for each time step because of appearing
    /// or disappearing particles. Setting an attribute using this function
    /// replaces any value previously set by ``NSISetAttribute()``.
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///               [`Context::create()`].
    ///
    /// * `time` – The time at which to set the value.
    ///
    /// * `args` – A [`std::slice`] of optional [`arg::Arg`] arguments.
    #[inline]
    pub fn set_attribute_at_time(
        &self,
        handle: impl Into<Vec<u8>>,
        time: f64,
        args: &arg::ArgSlice,
    ) {
        let args_out = arg::get_c_param_vec(args);

        unsafe {
            nsi_sys::NSISetAttributeAtTime(
                self.context,
                CString::new(handle).unwrap().as_ptr(),
                time,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    /// This function deletes any attribute with a name which matches
    /// the `name` argument on the specified object. There is no way to
    /// delete an attribute only for a specific time value.
    ///
    /// Deleting an attribute resets it to its default value.
    ///
    /// For example, after deleting the `transformationmatrix` attribute
    /// on a [`Node::Transform`], the transform will be an identity.
    /// Deleting a previously set attribute on a [`Node::Shader`] will
    /// default  to whatever is declared inside the shader.
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///               [`Context::create()`].
    ///
    /// * `name` – The name of the attribute to be deleted/reset.
    #[inline]
    pub fn delete_attribute(&self, handle: impl Into<Vec<u8>>, name: impl Into<Vec<u8>>) {
        unsafe {
            nsi_sys::NSIDeleteAttribute(
                self.context,
                CString::new(handle).unwrap().as_ptr(),
                CString::new(name).unwrap().as_ptr(),
            );
        }
    }

    /// These two function creates a connection between two elements.
    /// It is not an error to create a connection
    /// which already exists or to remove a connection which does not
    /// exist but the nodes on which the connection is performed must
    /// exist.
    ///
    /// # Arguments
    ///
    /// * `from` – The handle of the node from which the connection
    ///   is made.
    ///
    /// * `from_attr` – The name of the attribute from which the
    ///   connection is made. If this is an empty string then the
    ///   connection is made from the node instead of from a specific
    ///   attribute of the node.
    ///
    /// * `to` – The handle of the node to which the connection is made.
    ///
    /// * `to_attr` – The name of the attribute to which the connection
    ///   is made. If this is an empty string then the connection is
    ///   made to the node instead of to a specific attribute of the
    ///   node.
    ///
    /// # Optional Arguments
    ///
    /// * `"value"` – This can be used to change the value of a node's
    ///   attribute in some contexts. Refer to guidelines on
    ///   inter-object visibility for more information about the utility
    ///   of this parameter.
    ///
    /// * `"priority"` ([`arg::ArgData::Integer`]) – When connecting
    ///   attribute nodes, indicates in which order the nodes should be
    ///   considered when evaluating the value of an attribute.
    ///
    /// * `"strength"` ([`arg::ArgData::Integer`]) – A connection with a
    ///   `strength` greater than `0` will *block* the progression of a
    ///   recursive [`Context::delete()`].
    #[inline]
    pub fn connect(
        &self,
        from: impl Into<Vec<u8>>,
        from_attr: impl Into<Vec<u8>>,
        to: impl Into<Vec<u8>>,
        to_attr: impl Into<Vec<u8>>,
        args: &arg::ArgSlice,
    ) {
        let args_out = arg::get_c_param_vec(args);

        unsafe {
            nsi_sys::NSIConnect(
                self.context,
                CString::new(from).unwrap().as_ptr(),
                CString::new(from_attr).unwrap().as_ptr(),
                CString::new(to).unwrap().as_ptr(),
                CString::new(to_attr).unwrap().as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    /// This function removes a connection between two elements.
    ///
    /// The handle for either node may be the special value `".all"`.
    /// This will remove all connections which match the other three
    /// arguments.
    ///
    /// # Example
    /// ```
    /// // Create a rendering context.
    /// let ctx = nsi::Context::new(&[]).unwrap();
    /// // [...]
    /// // Disconnect everything from the scene's root.
    /// ctx.disconnect(".all", "", ".root", "");
    /// ```
    #[inline]
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
                CString::new(from).unwrap().as_ptr(),
                CString::new(from_attr).unwrap().as_ptr(),
                CString::new(to).unwrap().as_ptr(),
                CString::new(to_attr).unwrap().as_ptr(),
            );
        }
    }

    /// This function includes a block of interface calls from an
    /// external source into the current scene. It blends together the
    /// concepts of a straight file include, commonly known as an
    /// archive, with that of procedural include which is traditionally
    /// a compiled executable. Both are really the same idea expressed
    /// in a different language (note that for delayed procedural
    /// evaluation one should use a ([`Node::Procedural`]) node).
    ///
    /// The ɴsɪ adds a third option which sits in-between — (Lua
    /// scripts)[https://nsi.readthedocs.io/en/latest/lua-api.html]
    /// They are much more powerful than a simple included file yet
    /// they are also much easier to generate as they do not require
    /// compilation. It is, for example, very realistic to export a
    /// whole new script for every frame of an animation. It could also
    /// be done for every character in a frame. This gives great
    /// flexibility in how components of a scene are put together.
    ///
    /// The ability to load ɴsɪ commands straight from memory is also
    /// provided.
    ///
    /// # Optional Arguments
    ///
    /// * `"type"` ([`arg::ArgData::String`]) – The type of file which
    ///   will generate the interface calls. This can be one of:
    ///   * `"apistream"` – Read in an ɴsɪ stream. This requires either
    ///     `"filename"` or `"buffer"`/`"size"` arguments to be
    ///     specified too.
    ///
    ///   * `"lua"` – Execute a Lua script, either from file or inline.
    ///     See also
    ///     [how to evaluate a Lua script](https://nsi.readthedocs.io/en/latest/lua-api.html#luaapi-evaluation).
    ///
    ///   * `"dynamiclibrary"` – Execute native compiled code in a
    ///     loadable library. See
    ///     [dynamic library procedurals](https://nsi.readthedocs.io/en/latest/procedurals.html#section-procedurals)
    ///     for an implementation example in C.
    ///
    /// * `"filename"` ([`arg::ArgData::String`]) – The name of the
    ///   file which contains the interface calls to include.
    ///
    /// * `"script"` ([`arg::ArgData::String`]) – A valid Lua script to
    ///   execute when `"type"` is set to `"lua"`.
    ///
    /// * `"buffer"` ([`arg::ArgData::Pointer`])
    /// * `"size"` ([`arg::ArgData::Integer`]) – These two parameters
    ///   define a memory block that contain ɴsɪ commands to execute.
    ///
    /// * `"backgroundload"` ([`arg::ArgData::Integer`]) – If this is
    ///   nonzero, the object may be loaded in a separate thread, at
    ///   some later time. This requires that further interface calls
    ///   not directly reference objects defined in the included file.
    ///   The only guarantee is that the file will be loaded before
    ///   rendering begins.
    #[inline]
    pub fn evaluate(&self, args: &arg::ArgSlice) {
        let args_out = arg::get_c_param_vec(args);

        unsafe {
            nsi_sys::NSIEvaluate(
                self.context,
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    /// This function is the only control function of the api. It is
    /// responsible of starting, suspending and stopping the render.
    /// It also allows for synchronizing the render with interactive
    /// calls that might have been issued.
    ///
    /// # Optional Arguments
    ///
    /// * `"action"` ([`arg::ArgData::String`]) – Specifies the
    ///   operation to be performed, which should be one of the
    ///   following:
    ///   * `"start"` – This starts rendering the scene in the provided
    ///     context. The render starts in parallel and the control flow
    ///     is not blocked.
    ///
    ///   * `"wait"` – Wait for a render to finish.
    ///
    ///   * `"synchronize"` – For an interactive render, apply all the
    ///     buffered calls to scene’s state.
    ///
    ///   * `"suspend"` – Suspends render in the provided context.
    ///
    ///   * `"resume"` – Resumes a previously suspended render.
    ///
    ///   * `"stop"` – Stops rendering in the provided context without
    ///     destroying the scene
    /// * `"progressive"` ([`arg::ArgData::Integer`]) – If set to `1`,
    ///   render the image in a progressive fashion.
    ///
    /// * `"interactive"` ([`arg::ArgData::Integer`]) – If set to `1`,
    ///   the renderer will accept commands to edit scene’s state while
    ///   rendering. The difference with a normal render is that the
    ///   render task will not exit even if rendering is finished.
    ///   Interactive renders are by definition progressive.
    ///
    /// * `"frame"` – Specifies the frame number of this render.
    #[inline]
    pub fn render_control(&self, args: &arg::ArgSlice) {
        let args_out = arg::get_c_param_vec(args);

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
    #[inline]
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
    /// Describes one render layer to be connected to an `outputdriver`
    /// node. [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-outputlayer).
    OutputLayer,
    /// Describes how the view from a camera node will be rasterized
    /// into an `outputlayer` node. [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-screen).
    Screen,
}

impl Node {
    #[inline]
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

pub enum RenderStatus {
    Completed = 0,
    Aborted = 1,
    Synchronized = 2,
    Restarted = 3,
}
