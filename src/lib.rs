//! A Flexible, Modern API for Renderers
//!
//! The Nodal Scene Interface (ɴsɪ) is built around the concept of nodes.
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

use nsi_sys;
use std::{ffi::CString, ops::Drop, vec::Vec};

mod test;



static STR_ERROR: &str = "Found null byte in the middle of the string";

include!("argument.rs");

//type Handle = impl Into<Vec<u8>>;

/// An ɴsɪ context.
///
/// Also see the [ɴsɪ docmentation on context
/// handling](https://nsi.readthedocs.io/en/latest/c-api.html#context-handling).
pub struct Context {
    context: nsi_sys::NSIContext_t,
}

impl Context {
    pub fn new(args: &ArgVec) -> Self {
        Self {
            context: {
                if args.is_empty() {
                    unsafe { nsi_sys::NSIBegin(0, std::ptr::null()) }
                } else {
                    let mut args_out =
                        Vec::<nsi_sys::NSIParam_t>::new();
                    get_c_param_vec(args, &mut args_out);

                    unsafe {
                        nsi_sys::NSIBegin(
                            args_out.len() as i32,
                            args_out.as_ptr()
                                as *const nsi_sys::NSIParam_t,
                        )
                    }
                }
            },
        }
    }

    /// This function is used to create a new node.
    /// # Arguments
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
    /// * `args` - A vector of optional [`Arg`] parameters. *There are
    ///   no optional parameters defined as of now*.
    pub fn create(
        &self,
        handle: impl Into<Vec<u8>>,
        node_type: &Node,
        args: &ArgVec,
    ) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec(args, &mut args_out);

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
        get_c_param_vec(args, &mut args_out);

        unsafe {
            nsi_sys::NSIDelete(
                self.context,
                CString::new(handle.into()).expect(STR_ERROR).as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn set_attribute(
        &self,
        object: impl Into<Vec<u8>>,
        args: &ArgVec,
    ) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec(args, &mut args_out);

        unsafe {
            nsi_sys::NSISetAttribute(
                self.context,
                CString::new(object.into()).expect(STR_ERROR).as_ptr(),
                args_out.len() as i32,
                args_out.as_ptr() as *const nsi_sys::NSIParam_t,
            );
        }
    }

    pub fn set_attribute_at_time(
        &self,
        object: impl Into<Vec<u8>>,
        time: f64,
        args: &ArgVec,
    ) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec(args, &mut args_out);

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
        get_c_param_vec(args, &mut args_out);

        unsafe {
            nsi_sys::NSIConnect(
                self.context,
                CString::new(from.into()).expect(STR_ERROR).as_ptr(),
                CString::new(from_attr.into())
                    .expect(STR_ERROR)
                    .as_ptr(),
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
                CString::new(from_attr.into())
                    .expect(STR_ERROR)
                    .as_ptr(),
                CString::new(to.into()).expect(STR_ERROR).as_ptr(),
                CString::new(to_attr.into()).expect(STR_ERROR).as_ptr(),
            );
        }
    }

    pub fn evaluate(&self, args: &ArgVec) {
        let mut args_out = Vec::<nsi_sys::NSIParam_t>::new();
        get_c_param_vec(args, &mut args_out);

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
        get_c_param_vec(args, &mut args_out);

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

pub enum Node {
    Root, // = ".root",
    Global,
    Set,
    Shader,
    Attributes,
    Transform,
    Instances,
    Mesh,
    FaceSet,
    Curves,
    Particles,
    Procedural,
    Volume,
    Environment,
    Camera,
    /*OrthographicCamera,
    PerspectiveCamera,
    FisheyeCamera,
    CylindricalCamera,
    SphericalCamera,*/
    Outputdriver,
    Outputlayer,
    Screen,
}

impl Node {
    fn as_c_str(&self) -> &'static [u8] {
        match *self {
            Node::Root => b".root\0",
            Node::Global => b".global\0",
            Node::Set => b"set\0",
            Node::Shader => b"set\0",
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
            Node::Outputdriver => b"outputdriver\0",
            Node::Outputlayer => b"outputlayer\0",
            Node::Screen => b"screen\0",
        }
    }
}
