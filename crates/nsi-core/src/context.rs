//! # An ɴsɪ context.

// Needed for the example dode to build.
extern crate self as nsi;
use crate::{argument::*, *};
// std::slice is imported so the (doc) examples compile w/o hiccups.
use rclite::Arc;
#[allow(unused_imports)]
use std::{
    ffi::{c_char, CStr, CString},
    marker::PhantomData,
    ops::Drop,
    os::raw::{c_int, c_void},
};
use ustr::Ustr;

/// The actual context and a marker to hold on to callbacks
/// (closures)/references passed via [`set_attribute()`] or the like.
///
/// We wrap this in an [`Arc`] in [`Context`] to make sure drop() is only
/// called when the last clone ceases existing.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct InnerContext<'a> {
    context: NSIContext,
    // _marker needs to be invariant in 'a.
    // See "Making a struct outlive a parameter given to a method of
    // that struct": https://stackoverflow.com/questions/62374326/
    _marker: PhantomData<*mut &'a ()>,
}

unsafe impl<'a> Send for InnerContext<'a> {}
unsafe impl<'a> Sync for InnerContext<'a> {}

impl<'a> Drop for InnerContext<'a> {
    #[inline]
    fn drop(&mut self) {
        NSI_API.NSIEnd(self.context);
    }
}

/// # An ɴꜱɪ Context.
///
/// A `Context` is used to describe a scene to the renderer and request images
/// to be rendered from it.
///
/// ## Safety
/// A `Context` may be used in multiple threads at once.
///
/// ## Lifetime
/// A `Context` can be used without worrying about its lifetime until you want
/// to store it somewhere, e.g. in a struct.
///
/// The reason `Context` has an explicit lifetime is so that it can take
/// [`Reference`]s and [`Callback`]s (closures). These must be valid until the
/// context is dropped and this guarantee requires explicit lifetimes. When you
/// use a context directly this is not an issue but when you want to reference
/// it somewhere the same rules as with all references apply.
///
/// ## Further Reading
/// See the [ɴꜱɪ documentation on context
/// handling](https://nsi.readthedocs.io/en/latest/c-api.html#context-handling).
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Context<'a>(Arc<InnerContext<'a>>);

unsafe impl<'a> Send for Context<'a> {}
unsafe impl<'a> Sync for Context<'a> {}

impl<'a> From<Context<'a>> for NSIContext {
    #[inline]
    fn from(context: Context<'a>) -> Self {
        context.0.context
    }
}

impl<'a> Context<'a> {
    /// Creates an ɴsɪ context.
    ///
    /// Contexts may be used in multiple threads at once.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nsi_core as nsi;
    /// // Create rendering context that dumps to stdout.
    /// let ctx =
    ///     nsi::Context::new(Some(&[nsi::string!("streamfilename", "stdout")]))
    ///         .expect("Could not create ɴsɪ context.");
    /// ```
    /// # Error
    /// If this method fails for some reason, it returns [`None`].
    #[inline]
    pub fn new(args: Option<&ArgSlice<'_, 'a>>) -> Option<Self> {
        let (args_len, args_ptr, _args_out) = get_c_param_vec(args);

        let context = NSI_API.NSIBegin(args_len, args_ptr);

        if 0 == context {
            None
        } else {
            Some(Self(Arc::new(InnerContext {
                context,
                _marker: PhantomData,
            })))
        }
    }

    /// Creates a new node.
    ///
    /// # Arguments
    ///
    /// * `handle` -- A node handle. This string will uniquely identify the node
    ///   in the scene.
    ///
    ///   If the supplied handle matches an existing node, the function does
    /// nothing if all other parameters match the call which created that
    /// node. Otherwise, it emits an error. Note that handles need only be
    /// unique within a given [`Context`]. It is ok to reuse the same
    /// handle inside different [`Context`]s.
    ///
    /// * `node_type` -- The type of node to create. The crate has `&str`
    ///   constants for all [`node`]s that are in the official NSI
    ///   specification. As this parameter is just a string you can instance
    ///   other node types that a particular implementation may provide and
    ///   which are not part of the official specification.
    ///
    /// * `args` -- A [`slice`](std::slice) of optional [`Arg`] arguments.
    ///   *There are no optional arguments defined as of now*.
    ///
    /// ```
    /// # use nsi_core as nsi;
    /// // Create a context to send the scene to.
    /// let ctx = nsi::Context::new(None).unwrap();
    ///
    /// // Create an infinte plane.
    /// ctx.create("ground", nsi::PLANE, None);
    /// ```
    #[inline]
    pub fn create(
        &self,
        handle: &str,
        node_type: &str,
        args: Option<&ArgSlice<'_, 'a>>,
    ) {
        let handle = HandleString::from(handle);
        let node_type = Ustr::from(node_type);
        let (args_len, args_ptr, _args_out) = get_c_param_vec(args);

        NSI_API.NSICreate(
            self.0.context,
            handle.as_char_ptr(),
            node_type.as_char_ptr(),
            args_len,
            args_ptr,
        );
    }

    /// This function deletes a node from the scene. All connections to and from
    /// the node are also deleted.
    ///
    /// Note that it is not possible to delete the `.root` or the `.global`
    /// node.
    ///
    /// # Arguments
    ///
    /// * `handle` -- A handle to a node previously created with
    ///   [`create()`](Context::create()).
    ///
    /// * `args` -- A [`slice`](std::slice) of optional [`Arg`] arguments.
    ///
    /// # Optional Arguments
    ///
    /// * `"recursive"` ([`Integer`]) – Specifies whether deletion is recursive.
    ///   By default, only the specified node is deleted. If a value of `1` is
    ///   given, then nodes which connect to the specified node are recursively
    ///   removed. Unless they meet one of the following conditions:
    ///   * They also have connections which do not eventually lead to the
    ///     specified node.
    ///   * Their connection to the node to be deleted was created with a
    ///     `strength` greater than `0`.
    ///
    ///   This allows, for example, deletion of an entire shader network in a
    ///   single call.
    #[inline]
    pub fn delete(&self, handle: &str, args: Option<&ArgSlice<'_, 'a>>) {
        let handle = HandleString::from(handle);
        let (args_len, args_ptr, _args_out) = get_c_param_vec(args);

        NSI_API.NSIDelete(
            self.0.context,
            handle.as_char_ptr(),
            args_len,
            args_ptr,
        );
    }

    /// This functions sets attributes on a previously node.
    /// All optional arguments of the function become attributes of
    /// the node.
    ///
    /// On a [`shader`](`node::SHADER`), this function is used to set the
    /// implicitly defined shader arguments.
    ///
    /// Setting an attribute using this function replaces any value
    /// previously set by [`set_attribute()`](Context::set_attribute()) or
    /// [`set_attribute_at_time()`](Context::set_attribute_at_time()).
    ///
    /// To reset an attribute to its default value, use
    /// [`delete_attribute()`](Context::delete_attribute()).
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///   [`create()`](Context::create()).
    ///
    /// * `args` – A [`slice`](std::slice) of optional [`Arg`] arguments.
    #[inline]
    pub fn set_attribute(&self, handle: &str, args: &ArgSlice<'_, 'a>) {
        let handle = HandleString::from(handle);
        let (args_len, args_ptr, _args_out) = get_c_param_vec(Some(args));

        NSI_API.NSISetAttribute(
            self.0.context,
            handle.as_char_ptr(),
            args_len,
            args_ptr,
        );
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
    /// A notable  exception is the `P` attribute on [`particles`
    /// node](`node::PARTICLES`) which can be of different size for each
    /// time step because of appearing or disappearing particles. Setting an
    /// attribute using this function replaces any value previously set by
    /// [`set_attribute()`](Context::set_attribute()).
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///   [`create()`](Context::create()).
    ///
    /// * `time` – The time at which to set the value.
    ///
    /// * `args` – A [`slice`](std::slice) of optional [`Arg`] arguments.
    #[inline]
    pub fn set_attribute_at_time(
        &self,
        handle: &str,
        time: f64,
        args: &ArgSlice<'_, 'a>,
    ) {
        let handle = HandleString::from(handle);
        let (args_len, args_ptr, _args_out) = get_c_param_vec(Some(args));

        NSI_API.NSISetAttributeAtTime(
            self.0.context,
            handle.as_char_ptr(),
            time,
            args_len,
            args_ptr,
        );
    }

    /// This function deletes any attribute with a name which matches
    /// the `name` argument on the specified object. There is no way to
    /// delete an attribute only for a specific time value.
    ///
    /// Deleting an attribute resets it to its default value.
    ///
    /// For example, after deleting the `transformationmatrix` attribute
    /// on a [`transform` node](`node::TRANSFORM`), the transform will be an
    /// identity. Deleting a previously set attribute on a [`shader`
    /// node](`node::SHADER`) will default to whatever is declared inside
    /// the shader.
    ///
    /// # Arguments
    ///
    /// * `handle` – A handle to a node previously created with
    ///   [`create()`](Context::create()).
    ///
    /// * `name` – The name of the attribute to be deleted/reset.
    #[inline]
    pub fn delete_attribute(&self, handle: &str, name: &str) {
        let handle = HandleString::from(handle);
        let name = Ustr::from(name);

        NSI_API.NSIDeleteAttribute(
            self.0.context,
            handle.as_char_ptr(),
            name.as_char_ptr(),
        );
    }

    /// Create a connection between two elements.
    ///
    /// It is not an error to create a connection which already exists
    /// or to remove a connection which does not exist but the nodes
    /// on which the connection is performed must exist.
    ///
    /// # Arguments
    ///
    /// * `from` – The handle of the node from which the connection is made.
    ///
    /// * `from_attr` – The name of the attribute from which the connection is
    ///   made. If this is an empty string then the connection is made from the
    ///   node instead of from a specific attribute of the node.
    ///
    /// * `to` – The handle of the node to which the connection is made.
    ///
    /// * `to_attr` – The name of the attribute to which the connection is made.
    ///   If this is an empty string then the connection is made to the node
    ///   instead of to a specific attribute of the node.
    ///
    /// # Optional Arguments
    ///
    /// * `"value"` – This can be used to change the value of a node's attribute
    ///   in some contexts. Refer to guidelines on inter-object visibility for
    ///   more information about the utility of this parameter.
    ///
    /// * `"priority"` ([`Integer`]) – When connecting attribute nodes,
    ///   indicates in which order the nodes should be considered when
    ///   evaluating the value of an attribute.
    ///
    /// * `"strength"` ([`Integer`]) – A connection with a `strength` greater
    ///   than `0` will *block* the progression of a recursive
    ///   [`delete()`](Context::delete()).
    #[inline]
    pub fn connect(
        &self,
        from: &str,
        from_attr: &str,
        to: &str,
        to_attr: &str,
        args: Option<&ArgSlice<'_, 'a>>,
    ) {
        let from = HandleString::from(from);
        let from_attr = Ustr::from(from_attr);
        let to = HandleString::from(to);
        let to_attr = Ustr::from(to_attr);
        let (args_len, args_ptr, _args_out) = get_c_param_vec(args);

        NSI_API.NSIConnect(
            self.0.context,
            from.as_char_ptr(),
            from_attr.as_char_ptr(),
            to.as_char_ptr(),
            to_attr.as_char_ptr(),
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
    /// # Examples
    ///
    /// ```
    /// # use nsi_core as nsi;
    /// // Create a rendering context.
    /// let ctx = nsi::Context::new(None).unwrap();
    /// // [...]
    /// // Disconnect everything from the scene's root.
    /// ctx.disconnect(".all", "", ".root", "");
    /// ```
    #[inline]
    pub fn disconnect(
        &self,
        from: &str,
        from_attr: &str,
        to: &str,
        to_attr: &str,
    ) {
        let from = HandleString::from(from);
        let from_attr = Ustr::from(from_attr);
        let to = HandleString::from(to);
        let to_attr = Ustr::from(to_attr);

        NSI_API.NSIDisconnect(
            self.0.context,
            from.as_char_ptr(),
            from_attr.as_char_ptr(),
            to.as_char_ptr(),
            to_attr.as_char_ptr(),
        );
    }

    /// This function includes a block of interface calls from an external
    /// source into the current scene. It blends together the concepts of a
    /// file include, commonly known as an *archive*, with that of
    /// procedural include which is traditionally a compiled executable. Both
    /// are the same idea expressed in a different language.
    ///
    /// Note that for delayed procedural evaluation you should use a
    /// [`procedural` node](node::PROCEDURAL).
    ///
    /// The ɴsɪ adds a third option which sits in-between — [Lua
    /// scripts](https://nsi.readthedocs.io/en/latest/lua-api.html). They are more powerful than a
    /// simple included file yet they are also easier to generate as they do not
    /// require compilation.
    ///
    /// For example, it is realistic to export a whole new script for every
    /// frame of an animation. It could also be done for every character in
    /// a frame. This gives great flexibility in how components of a scene
    /// are put together.
    ///
    /// The ability to load ɴsɪ commands from memory is also provided.
    ///
    /// # Optional Arguments
    ///
    /// * `"type"` ([`String`]) – The type of file which will generate the
    ///   interface calls. This can be one of:
    ///   * `"apistream"` – Read in an ɴsɪ stream. This requires either
    ///     `"filename"` or `"buffer"`/`"size"` arguments to be specified too.
    ///
    ///   * `"lua"` – Execute a Lua script, either from file or inline. See also
    ///     [how to evaluate a Lua script](https://nsi.readthedocs.io/en/latest/lua-api.html#luaapi-evaluation).
    ///
    ///   * `"dynamiclibrary"` – Execute native compiled code in a loadable library. See
    ///     [dynamic library procedurals](https://nsi.readthedocs.io/en/latest/procedurals.html#section-procedurals)
    ///     for an implementation example in C.
    ///
    /// * `"filename"` ([`String`]) – The name of the file which contains the
    ///   interface calls to include.
    ///
    /// * `"script"` ([`String`]) – A valid Lua script to execute when `"type"`
    ///   is set to `"lua"`.
    ///
    /// * `"buffer"` ([`Pointer`])
    /// * `"size"` ([`Integer`]) – These two parameters define a memory block
    ///   that contain ɴsɪ commands to execute.
    ///
    /// * `"backgroundload"` ([`Integer`]) – If this is nonzero, the object may
    ///   be loaded in a separate thread, at some later time. This requires that
    ///   further interface calls not directly reference objects defined in the
    ///   included file. The only guarantee is that the file will be loaded
    ///   before rendering begins.
    #[inline]
    pub fn evaluate(&self, args: &ArgSlice<'_, 'a>) {
        let (args_len, args_ptr, _args_out) = get_c_param_vec(Some(args));

        NSI_API.NSIEvaluate(self.0.context, args_len, args_ptr);
    }

    /// This function is the only control function of the API.
    ///
    /// It is responsible of starting, suspending and stopping the render. It
    /// also allows for synchronizing the render with interactive calls that
    /// might have been issued.
    ///
    /// # Optional Arguments
    ///
    /// * `"action"` ([`String`]) – Specifies the operation to be performed,
    ///   which should be one of the following:
    ///   * `"start"` – This starts rendering the scene in the provided context.
    ///     The render starts in parallel and the control flow is not blocked.
    ///
    ///   * `"wait"` – Wait for a render to finish.
    ///
    ///   * `"synchronize"` – For an interactive render, apply all the buffered
    ///     calls to scene’s state.
    ///
    ///   * `"suspend"` – Suspends render in the provided context.
    ///
    ///   * `"resume"` – Resumes a previously suspended render.
    ///
    ///   * `"stop"` – Stops rendering in the provided context without
    ///     destroying the scene.
    /// * `"progressive"` ([`Integer`]) – If set to `1`, render the image in a
    ///   progressive fashion.
    ///
    /// * `"interactive"` ([`Integer`]) – If set to `1`, the renderer will
    ///   accept commands to edit scene’s state while rendering. The difference
    ///   with a normal render is that the render task will not exit even if
    ///   rendering is finished. Interactive renders are by definition
    ///   progressive.
    ///
    /// * `"frame"` – Specifies the frame number of this render.
    #[inline]
    pub fn render_control(&self, args: &ArgSlice<'_, 'a>) {
        let (_, _, mut args_out) = get_c_param_vec(Some(args));

        let fn_pointer: nsi_sys::NSIRenderStopped =
            Some(render_status as extern "C" fn(*mut c_void, i32, i32));

        if let Some(arg) =
            args.iter().find(|arg| Ustr::from("callback") == arg.name)
        {
            args_out.push(nsi_sys::NSIParam {
                name: Ustr::from("stoppedcallback").as_char_ptr(),
                data: &fn_pointer as *const _ as _,
                type_: NSIType::Pointer as _,
                arraylength: 0,
                count: 1,
                flags: 0,
            });
            args_out.push(nsi_sys::NSIParam {
                name: Ustr::from("stoppedcallbackdata").as_char_ptr(),
                data: &arg.data.as_c_ptr() as *const _ as _,
                type_: NSIType::Pointer as _,
                arraylength: 1,
                count: 1,
                flags: 0,
            });
        }

        NSI_API.NSIRenderControl(
            self.0.context,
            args_out.len() as _,
            args_out.as_ptr(),
        );
    }
}

/// The status of a *interactive* render session.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, num_enum::FromPrimitive)]
pub enum RenderStatus {
    #[num_enum(default)]
    Completed = nsi_sys::NSIStoppingStatus::RenderCompleted as _,
    Aborted = nsi_sys::NSIStoppingStatus::RenderAborted as _,
    Synchronized = nsi_sys::NSIStoppingStatus::RenderSynchronized as _,
    Restarted = nsi_sys::NSIStoppingStatus::RenderRestarted as _,
}

/// A closure which is called to inform about the status of an ongoing render.
///
/// It is passed to ɴsɪ via [`render_control()`](Context::render_control())’s
/// `"callback"` argument.
///
/// # Examples
///
/// ```
/// # use nsi_core as nsi;
/// # let ctx = nsi::Context::new(None).unwrap();
/// let status_callback = nsi::context::StatusCallback::new(
///     |_: &nsi::context::Context, status: nsi::context::RenderStatus| {
///         println!("Status: {:?}", status);
///     },
/// );
///
/// ctx.render_control(&[
///     nsi::string!("action", "start"),
///     nsi::callback!("callback", status_callback),
/// ]);
/// ```
pub trait FnStatus<'a>: Fn(
    // The [`Context`] for which this closure was called.
    &Context,
    // Status of interactive render session.
    RenderStatus,
)
+ 'a {}

#[doc(hidden)]
impl<
        'a,
        T: Fn(&Context, RenderStatus)
            + 'a
            + for<'r, 's> Fn(&'r context::Context<'s>, RenderStatus),
    > FnStatus<'a> for T
{
}

// FIXME once trait aliases are in stable.
/*
trait FnStatus<'a> = FnMut(
    // Status of interactive render session.
    status: RenderStatus
    )
    + 'a
*/

/// Wrapper to pass a [`FnStatus`] closure to a [`Context`].
pub struct StatusCallback<'a>(Box<Box<dyn FnStatus<'a>>>);

impl<'a> StatusCallback<'a> {
    pub fn new<F>(fn_status: F) -> Self
    where
        F: FnStatus<'a>,
    {
        StatusCallback(Box::new(Box::new(fn_status)))
    }
}

impl CallbackPtr for StatusCallback<'_> {
    #[doc(hidden)]
    fn to_ptr(self) -> *const core::ffi::c_void {
        Box::into_raw(self.0) as *const _ as _
    }
}

// Trampoline function for the FnStatus callback.
#[no_mangle]
pub(crate) extern "C" fn render_status(
    payload: *mut c_void,
    context: nsi_sys::NSIContext,
    status: c_int,
) {
    if !payload.is_null() {
        let fn_status =
            unsafe { Box::from_raw(payload as *mut Box<dyn FnStatus>) };
        let ctx = Context(Arc::new(InnerContext {
            context,
            _marker: PhantomData,
        }));

        fn_status(&ctx, status.into());

        // We must not call drop() on this context.
        // This is safe as Context doesn't allocate and this one is on the stack
        // anyway.
        std::mem::forget(ctx);
    }
}
