#![feature(specialization)]
//#![feature(const_generics)]
#![feature(type_alias_impl_trait)]

//#![allow(non_upper_case_globals)]
//#![allow(non_camel_case_types)]
//#![allow(non_snake_case)]

//#![feature(associated_type_bounds)]

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

    fn set_attribute(
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

    fn set_attribute_at_time(
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

    fn connect(
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

    fn disconnect(
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

    fn evaluate(&mut self, args: &ArgVec) {
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

    fn render_control(&mut self, args: &ArgVec) {
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

/*
#[test]
test_create() {
    let nsi_context = nsi::Context::new(no_args!)

    nsi_context.create(
        "main_camera",
        "perpectivecamera",
        build_parameter_list!(
            ( "depthoffield.enable", true ),
            ( "depthoffield.fstop", f_stop ),
            ( "depthoffield.focallength", focal_length ),
            ( "depthoffield.focaldistance", focal_distance ),
            ( "depthoffield.aperture.enable", true ),
            ( "depthoffield.aperture.sides", {
                    match camera_stream_values[ &ae_sys::AEGP_LayerStream_IRIS_SHAPE ] {
                        1 => 4 // 1 == fast rectangle == 4 sides
                        _ =>
                    }
                }
            ),
            ( "depthoffield.aperture.angle", camera_stream_values[ &ae_sys::AEGP_LayerStream_IRIS_ROTATION ] )
         )
    );

}*/

/*fn param( &mut self, Arg) {

}*/

/*
macro_rules! parameter_list {
    // The pattern for a single `eval`
    (( $e:expr, $e:expr)) => {{
        {
            let val: usize = $e; // Force types to be integers
            println!("{} = {}", stringify!{$e}, val);
        }
    }};

    // Decompose multiple `eval`s recursively
    (eval $e:expr, $(eval $es:expr),+) => {{
        calculate! { eval $e }
        calculate! { $(eval $es),+ }
    }};
}*/

macro_rules! make_list(
    () => (
        None
    );
    ( $x:expr $( , $more:expr )* ) => (
        Node::new($x, make_list!( $( $more ),* ))
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]

    fn it_works() {
        let nsi_context: Context;

        /*let foo: Arg = {
            name: ""
        };*/

        let nsi_context = nsi::Context::new(&ArgVec::new());

        //   .param( "type", "cloud" )
        //    .param( errorhandler, my_error_handler )

        /*nsi_context
        .set_attribute("handle")
        .param("fov", 45.0)
        .param("depthoffield.fstop", 4.0);*/

        nsi_context
            .render_control(&ArgVec::new(Arg::new("action", "start")));
    }

    // nsi::Attribute::new("handle")
    //    .add( "fov", 45.0f )
    //    .add( "depthoffield.fstop", 4.0 )
}
