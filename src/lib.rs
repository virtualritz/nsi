#![feature(specialization)]

static STR_ERROR: &str = "Found null byte in the middle of the string";

extern crate nsi_sys;

include!("argument.rs");

use std::{ops::Drop, vec::Vec};

//type Handle = impl Into<Vec<u8>>;

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
    ///   created that node.
    ///   Otherwise, it emits an error. Note that handles need only be
    ///   unique within a given interface context. It is acceptable to
    ///   reuse the same handle inside different contexts.
    ///
    /// * `type` - The type of node to create.
    ///
    /// * `args` - A vector of optional [`Arg`] parameters. *There are
    ///   no optional parameters defined as of now*.
    pub fn create(
        &mut self,
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

    pub fn delete(
        &mut self,
        handle: impl Into<Vec<u8>>,
        args: &ArgVec,
    ) {
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
        &mut self,
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
        &mut self,
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
        &mut self,
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
        &mut self,
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

    pub fn evaluate(&mut self, args: &ArgVec) {
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

    pub fn render_control(&mut self, args: &ArgVec) {
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
            //(len, arg_list) = nsi_sys_arg_list!( args );
            nsi_sys::NSIEnd(self.context);
        }
    }
}

pub enum Node {
    Root,
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
