//! A flexible, modern API for offline 3D renderers
//!
//! The [Nodal Scene Interface](https://nsi.readthedocs.io/) (ɴsɪ) is
//! built around the concept of nodes.
//! Each node has a unique handle to identify it and a type which
//! describes its intended function in the scene. Nodes are abstract
//! containers for data. The interpretation depends on the node type.
//! Nodes can also be [connected to each other](https://nsi.readthedocs.io/en/latest/guidelines.html#basic-scene-anatomy)
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
//! attributes to nodes. The validity of such connections depends on
//! the types of the nodes involved.
//!
//! What we refer to as the ɴsɪ has two major components:
//!
//! 1.  Methods to create nodes, attributes and their connections.
//!     These are attached to a rendering [`Context`].
//!
//! 2.  Nodes of different [`NodeType`]s understood by the renderer.
//!
//! Much of the complexity and expressiveness of the interface comes
//! from
//! [the supported nodes](https://nsi.readthedocs.io/en/latest/nodes.html).
//!
//! The first part was kept deliberately simple to make it easy to
//! support multiple ways of creating nodes.
//!
//! ## Features
//! The 3Delight dynamic library (`lib3delight`) can either be linked to
//! during build or loaded at runtime.
//! By default the lib is loaded at runtime. This has several
//! advantages:
//! 1. If you ship your application or library you can ship it without
//!    the library. It can still run and will print an informative error
//!    if the library cannot be loaded.
//! 2. A user can install an updated version of the renderer and stuff
//!    will ‘just work’.
//!
//! * Dynamically link against `lib3delight`.
//!   * `lib3delight` becomes a depdency. If it cannot't be found your
//!     lib/app will not load/start.
//!   * The feature is called `link_lib3delight`.
//! * Download `lib3delight` during build.
//!   * `lib3delight` is downloaded during build. Note that this may be
//!     an outdated version. This feature mainly exists for CI purposes.
//!   * The feature is called `download_lib3delight`.
#![allow(non_snake_case)]

extern crate self as nsi;
// slice is imported so the (doc) examples compile w/o hiccups.
#[allow(unused_imports)]
use std::{ffi::CString, marker::PhantomData, ops::Drop, slice, vec::Vec};

#[cfg(not(feature = "link_lib3delight"))]
#[macro_use]
extern crate dlopen_derive;

use nsi_sys::*;

trait Api {
    fn NSIBegin(&self, nparams: ::std::os::raw::c_int, params: *const NSIParam_t) -> NSIContext_t;
    fn NSIEnd(&self, ctx: NSIContext_t);
    fn NSICreate(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDelete(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSISetAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        name: *const ::std::os::raw::c_char,
    );
    fn NSIConnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDisconnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
    );
    fn NSIEvaluate(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIRenderControl(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
}

#[cfg(not(feature = "link_lib3delight"))]
mod dynamic;
#[cfg(feature = "link_lib3delight")]
mod linked;

#[cfg(not(feature = "link_lib3delight"))]
use self::dynamic as api;
#[cfg(feature = "link_lib3delight")]
use self::linked as api;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref NSI_API: api::ApiImpl = api::ApiImpl::new().unwrap();
}

#[macro_use]
mod argument;
#[allow(unused_imports)]
use crate::NodeType::*;
#[allow(unused_imports)]
use arg::{Arg, ArgData::*};
pub use argument::*;
mod tests;

//type Handle = impl Into<Vec<u8>>;

/// An ɴsɪ context.
///
/// # Lifetime
/// A context can be used without worrying about its lifetime
/// until you want to store it somewhere, e.g. in a struct.
///
/// The reason a context has an explicit lifetime is so that it can
/// take [`Reference`]s. These references must be valid until the
/// context is dropped and this guarantee requires explicit lifetimes.
/// When you use a context directly this is not an issue
/// but when you want to reference it somewhere the same rules
/// as with all references apply.
///
/// # Further Reading
/// See the [ɴsɪ docmentation on context
/// handling](https://nsi.readthedocs.io/en/latest/c-api.html#context-handling).
#[derive(Debug, Hash, PartialEq)]
pub struct Context<'a> {
    context: NSIContext_t,
    // _marker needs to be invariant in 'a.
    // See "Making a struct outlive a parameter given to a method of
    // that struct": https://stackoverflow.com/questions/62374326/
    _marker: PhantomData<*mut &'a ()>,
}

impl<'a> From<NSIContext_t> for Context<'a> {
    #[inline]
    fn from(context: NSIContext_t) -> Self {
        Self {
            context,
            _marker: PhantomData,
        }
    }
}

impl<'a> Context<'a> {
    //count: HashMap<NSIContext_t

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
    pub fn new(args: &arg::ArgSlice<'_, 'a>) -> Option<Self> {
        match {
            let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);
            NSI_API.NSIBegin(args_len, args_ptr)
        } {
            0 => None,
            ref c => Some(Self {
                context: *c,
                _marker: PhantomData,
            }),
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
    /// * `node_type` – The type of node to create. You can use
    ///   [`NodeType`] to create nodes that are in the official NSI
    ///   specificaion.
    ///   As this parameter is just a string you can instance other
    ///   node types that are a particualr implementation may provide
    ///   and which are not in the official spec.
    ///
    /// * `args` – A [`slice`] of optional [`Arg`] arguments. *There are
    ///   no optional arguments defined as of now*.
    ///
    /// ```
    /// // Create a context to send the scene to.
    /// let ctx = nsi::Context::new(&[]).unwrap();
    ///
    /// // Create an infinte plane.
    /// ctx.create("ground", nsi::NodeType::Plane, &[]);
    /// ```
    #[inline]
    pub fn create(
        &self,
        handle: impl Into<Vec<u8>>,
        node_type: impl Into<Vec<u8>>,
        args: &arg::ArgSlice<'_, 'a>,
    ) {
        let handle = CString::new(handle).unwrap();
        let node_type = CString::new(node_type).unwrap();
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSICreate(
            self.context,
            handle.as_ptr(),
            node_type.as_ptr(),
            args_len,
            args_ptr,
        )
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
    /// * `args` – A [`slice`] of optional [`Arg`] arguments.
    ///
    /// # Optional Arguments
    ///
    /// * `"recursive"` ([`Integer`]) – Specifies whether
    ///   deletion is recursive. By default, only the specified node is
    ///   deleted. If a value of `1` is given, then nodes which connect
    ///   to the specified node are recursively removed. Unless they
    ///   meet one of the following conditions:
    ///   * They also have connections which do not eventually lead to
    ///     the specified node.
    ///   * Their connection to the node to be deleted was created with
    ///     a `strength` greater than `0`.
    ///
    ///   This allows, for example, deletion of an entire shader network in a single call.
    #[inline]
    pub fn delete(&self, handle: impl Into<Vec<u8>>, args: &arg::ArgSlice<'_, 'a>) {
        let handle = CString::new(handle).unwrap();
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSIDelete(self.context, handle.as_ptr(), args_len, args_ptr);
    }

    /// This functions sets attributes on a previously node.
    /// All optional arguments of the function become attributes of
    /// the node.
    ///
    /// On a [`Shader`], this function is used to set the implicitly
    /// defined shader arguments.
    ///
    /// Setting an attribute using this function replaces any value
    /// previously set by [`Context::set_attribute()`] or
    /// [`Context::set_attribute_at_time()`].
    /// To reset an attribute to its default value, use
    /// [`Context::delete_attribute()`]).
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///              [`Context::create()`].
    ///
    /// * `args` – A [`slice`] of optional [`Arg`] arguments.
    #[inline]
    pub fn set_attribute(&self, handle: impl Into<Vec<u8>>, args: &arg::ArgSlice<'_, 'a>) {
        let handle = CString::new(handle).unwrap();
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSISetAttribute(self.context, handle.as_ptr(), args_len, args_ptr);
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
    /// A notable  exception is the `P` attribute on (particles)[`Particles`]
    /// which can be of different size for each time step because of appearing
    /// or disappearing particles. Setting an attribute using this function
    /// replaces any value previously set by [`Context::set_attribute()`].
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///               [`Context::create()`].
    ///
    /// * `time` – The time at which to set the value.
    ///
    /// * `args` – A [`slice`] of optional [`Arg`] arguments.
    #[inline]
    pub fn set_attribute_at_time(
        &self,
        handle: impl Into<Vec<u8>>,
        time: f64,
        args: &arg::ArgSlice<'_, 'a>,
    ) {
        let handle = CString::new(handle).unwrap();
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSISetAttributeAtTime(self.context, handle.as_ptr(), time, args_len, args_ptr);
    }

    /// This function deletes any attribute with a name which matches
    /// the `name` argument on the specified object. There is no way to
    /// delete an attribute only for a specific time value.
    ///
    /// Deleting an attribute resets it to its default value.
    ///
    /// For example, after deleting the `transformationmatrix` attribute
    /// on a [`Transform`] node, the transform will be an identity.
    /// Deleting a previously set attribute on a [`Shader`] node will
    /// default to whatever is declared inside the shader.
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///               [`Context::create()`].
    ///
    /// * `name` – The name of the attribute to be deleted/reset.
    #[inline]
    pub fn delete_attribute(&self, handle: impl Into<Vec<u8>>, name: impl Into<Vec<u8>>) {
        let handle = CString::new(handle).unwrap();
        let name = CString::new(name).unwrap();

        NSI_API.NSIDeleteAttribute(self.context, handle.as_ptr(), name.as_ptr());
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
    /// * `"priority"` ([`Integer`]) – When connecting
    ///   attribute nodes, indicates in which order the nodes should be
    ///   considered when evaluating the value of an attribute.
    ///
    /// * `"strength"` ([`Integer`]) – A connection with a
    ///   `strength` greater than `0` will *block* the progression of a
    ///   recursive [`Context::delete()`].
    #[inline]
    pub fn connect(
        &self,
        from: impl Into<Vec<u8>>,
        from_attr: impl Into<Vec<u8>>,
        to: impl Into<Vec<u8>>,
        to_attr: impl Into<Vec<u8>>,
        args: &arg::ArgSlice<'_, 'a>,
    ) {
        let from = CString::new(from).unwrap();
        let from_attr = CString::new(from_attr).unwrap();
        let to = CString::new(to).unwrap();
        let to_attr = CString::new(to_attr).unwrap();
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSIConnect(
            self.context,
            from.as_ptr(),
            from_attr.as_ptr(),
            to.as_ptr(),
            to_attr.as_ptr(),
            args_len,
            args_ptr,
        );
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
        let from = CString::new(from).unwrap();
        let from_attr = CString::new(from_attr).unwrap();
        let to = CString::new(to).unwrap();
        let to_attr = CString::new(to_attr).unwrap();

        NSI_API.NSIDisconnect(
            self.context,
            from.as_ptr(),
            from_attr.as_ptr(),
            to.as_ptr(),
            to_attr.as_ptr(),
        );
    }

    /// This function includes a block of interface calls from an
    /// external source into the current scene. It blends together the
    /// concepts of a straight file include, commonly known as an
    /// archive, with that of procedural include which is traditionally
    /// a compiled executable. Both are really the same idea expressed
    /// in a different language (note that for delayed procedural
    /// evaluation one should use a ([`Procedural`]) node).
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
    /// * `"type"` ([`String`]) – The type of file which
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
    /// * `"filename"` ([`String`]) – The name of the
    ///   file which contains the interface calls to include.
    ///
    /// * `"script"` ([`String`]) – A valid Lua script to
    ///   execute when `"type"` is set to `"lua"`.
    ///
    /// * `"buffer"` ([`Pointer`])
    /// * `"size"` ([`Integer`]) – These two parameters
    ///   define a memory block that contain ɴsɪ commands to execute.
    ///
    /// * `"backgroundload"` ([`Integer`]) – If this is
    ///   nonzero, the object may be loaded in a separate thread, at
    ///   some later time. This requires that further interface calls
    ///   not directly reference objects defined in the included file.
    ///   The only guarantee is that the file will be loaded before
    ///   rendering begins.
    #[inline]
    pub fn evaluate(&self, args: &arg::ArgSlice<'_, 'a>) {
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSIEvaluate(self.context, args_len, args_ptr);
    }

    /// This function is the only control function of the api. It is
    /// responsible of starting, suspending and stopping the render.
    /// It also allows for synchronizing the render with interactive
    /// calls that might have been issued.
    ///
    /// # Optional Arguments
    ///
    /// * `"action"` ([`String`]) – Specifies the
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
    /// * `"progressive"` ([`Integer`]) – If set to `1`,
    ///   render the image in a progressive fashion.
    ///
    /// * `"interactive"` ([`Integer`]) – If set to `1`,
    ///   the renderer will accept commands to edit scene’s state while
    ///   rendering. The difference with a normal render is that the
    ///   render task will not exit even if rendering is finished.
    ///   Interactive renders are by definition progressive.
    ///
    /// * `"frame"` – Specifies the frame number of this render.
    #[inline]
    pub fn render_control(&self, args: &arg::ArgSlice<'_, 'a>) {
        let (args_len, args_ptr, _args_out) = arg::get_c_param_vec(args);

        NSI_API.NSIRenderControl(self.context, args_len, args_ptr);
    }
}

impl<'a> Drop for Context<'a> {
    #[inline]
    fn drop(&mut self) {
        NSI_API.NSIEnd(self.context);
    }
}

/// The type for a node in the ɴsɪ scene graph.
///
/// This will just convert into a `Vec<u8>` of the string representing
/// the node type when you use it.
pub enum NodeType {
    /// The scene’s root (`".root"`).
    /// [Documentation](https://nsi.readthedocs.io/en/latest/nodes.html#node-root).
    Root, // = ".root",
    /// Global settings node (`".global"`).
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

impl From<NodeType> for Vec<u8> {
    #[inline]
    fn from(node_type: NodeType) -> Self {
        match node_type {
            NodeType::Root => b".root".to_vec(),
            NodeType::Global => b".global".to_vec(),
            NodeType::Set => b"set".to_vec(),
            NodeType::Plane => b"plane".to_vec(),
            NodeType::Shader => b"shader".to_vec(),
            NodeType::Attributes => b"attributes".to_vec(),
            NodeType::Transform => b"transform".to_vec(),
            NodeType::Instances => b"instances".to_vec(),
            NodeType::Mesh => b"mesh".to_vec(),
            NodeType::FaceSet => b"faceset".to_vec(),
            NodeType::Curves => b"curves".to_vec(),
            NodeType::Particles => b"particles".to_vec(),
            NodeType::Procedural => b"procedural".to_vec(),
            NodeType::Volume => b"volume".to_vec(),
            NodeType::Environment => b"environment".to_vec(),
            NodeType::Camera => b"camera".to_vec(),
            NodeType::OrthographicCamera => b"orthographiccamera".to_vec(),
            NodeType::PerspectiveCamera => b"perspectivecamera".to_vec(),
            NodeType::FisheyeCamera => b"fisheyecamera".to_vec(),
            NodeType::CylindricalCamera => b"cylindricalcamera".to_vec(),
            NodeType::SphericalCamera => b"sphericalcamera".to_vec(),
            NodeType::OutputDriver => b"outputdriver".to_vec(),
            NodeType::OutputLayer => b"outputlayer".to_vec(),
            NodeType::Screen => b"screen".to_vec(),
        }
    }
}

/// The status of a *interactive* render session.
pub enum RenderStatus {
    Completed = 0,
    Aborted = 1,
    Synchronized = 2,
    Restarted = 3,
}
