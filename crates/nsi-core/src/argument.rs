//! # Optional Arguments Passed to Methods of an ɴsɪ Context.
use enum_dispatch::enum_dispatch;
use nsi_sys::*;
use std::{
    ffi::{c_void, CString},
    marker::PhantomData,
};
use ustr::Ustr;

// Needed for docs.
#[allow(unused_imports)]
use crate::*;

#[inline(always)]
pub(crate) fn get_c_param_vec(
    args: Option<&ArgSlice>,
) -> (i32, *const NSIParam, Vec<NSIParam>) {
    let args = match args {
        Some(args) => args
            .iter()
            .map(|arg| NSIParam {
                name: arg.name.as_char_ptr(),
                data: arg.data.as_c_ptr(),
                type_: arg.data.type_() as _,
                arraylength: arg.array_length as _,
                count: (arg.data.len() / arg.array_length) as _,
                flags: arg.flags as _,
            })
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };

    (args.len() as _, args.as_ptr(), args)
}

/// A slice of (optional) arguments passed to a method of
/// [`Context`](context::Context).
pub type ArgSlice<'a, 'b> = [Arg<'a, 'b>];

/// A vector of (optional) arguments passed to a method of
/// [`Context`](context::Context).
pub type ArgVec<'a, 'b> = Vec<Arg<'a, 'b>>;

/// An (optional) argument passed to a method of
/// [`Context`](context::Context).
#[derive(Debug)]
pub struct Arg<'a, 'b> {
    pub(crate) name: Ustr,
    pub(crate) data: ArgData<'a, 'b>,
    // length of each element if an array type
    pub(crate) array_length: usize,
    // number of elements
    pub(crate) flags: i32,
}

impl<'a, 'b> Arg<'a, 'b> {
    #[inline]
    pub fn new(name: &str, data: ArgData<'a, 'b>) -> Self {
        Arg {
            name: Ustr::from(name),
            data,
            array_length: 1,
            flags: 0,
        }
    }

    /// Sets the length of the argument for each element.
    #[inline]
    pub fn array_len(mut self, length: usize) -> Self {
        self.array_length = length;
        self.flags |= NSIParamFlags::IsArray.bits();
        self
    }

    /// Marks this argument as having per-face granularity.
    #[inline]
    pub fn per_face(mut self) -> Self {
        self.flags |= NSIParamFlags::PerFace.bits();
        self
    }

    /// Marks this argument as having per-vertex granularity.
    #[inline]
    pub fn per_vertex(mut self) -> Self {
        self.flags |= NSIParamFlags::PerVertex.bits();
        self
    }

    /// Marks this argument as to be interpolated linearly.
    #[inline]
    pub fn linear_interpolation(mut self) -> Self {
        self.flags |= NSIParamFlags::InterpolateLinear.bits();
        self
    }
}

#[enum_dispatch(ArgData)]
pub(crate) trait ArgDataMethods {
    //const TYPE: Type;
    fn type_(&self) -> Type;
    fn len(&self) -> usize;
    fn as_c_ptr(&self) -> *const c_void;
}

/// A variant describing data passed to the renderer.
///
/// # Lifetimes
/// Lifetime `'a` is for any tuple or array type as these are
/// passed as references and only need to live as long as the
/// function call where they get passed.
///
/// Lifetime `'b` is for the arbitrary reference type. This is
/// pegged to the lifetime of the [`Context`](crate::context::Context).
/// Use this to pass arbitrary Rust data through the FFI boundary.
#[enum_dispatch]
#[derive(Debug)]
pub enum ArgData<'a, 'b> {
    /// Single [`f32`] value.
    Float,
    Floats(Floats<'a>),
    /// Single [`f64`] value.
    Double,
    Doubles(Doubles<'a>),
    /// Single [`i32`] value.
    Integer,
    /// An [`i32`] array.
    Integers(Integers<'a>),
    /// A [`String`].
    String(String),
    /// A [`String`] array.
    Strings(Strings),
    /// Color in linear space, given as a red, green, blue triplet
    /// of [`f32`] values; usually in the range `0..1`.
    Color(Color<'a>),
    /// An array of colors.
    Colors(Colors<'a>),
    /// Point, given as three [`f32`] values.
    Point(Point<'a>),
    Points(Points<'a>),
    /// Vector, given as three [`f32`] values.
    Vector(Vector<'a>),
    Vectors(Vectors<'a>),
    /// Normal vector, given as three [`f32`] values.
    Normal(Normal<'a>),
    Normals(Normals<'a>),
    /// Row-major, 4×4 transformation matrix, given as 16 [`f32`] values.
    Matrix(Matrix<'a>),
    Matrices(Matrices<'a>),
    /// Row-major, 4×4 transformation matrix, given as 16 [`f64`] values.
    DoubleMatrix(DoubleMatrix<'a>),
    DoubleMatrices(DoubleMatrices<'a>),
    /// Reference to arbitrary data.
    Reference(Reference<'b>),
    References(References<'b>),
    /// Callback.
    Callback(Callback<'b>),
    /// Raw (`*const T`) pointer.
    Pointer,
    Pointers(Pointers<'a>),
}

macro_rules! nsi_data_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
        #[derive(Debug)]
        pub struct $name {
            data: $type,
        }

        impl $name {
            pub fn new(data: $type) -> Self {
                Self { data }
            }
        }

        impl ArgDataMethods for $name {
            fn type_(&self) -> Type {
                $nsi_type
            }

            fn len(&self) -> usize {
                1
            }

            fn as_c_ptr(&self) -> *const c_void {
                &self.data as *const $type as _
            }
        }
    };
}

macro_rules! nsi_data_array_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
        #[derive(Debug)]
        pub struct $name<'a> {
            data: &'a [$type],
        }

        impl<'a> $name<'a> {
            pub fn new(data: &'a [$type]) -> Self {
                debug_assert!(data.len() % $nsi_type.elemensize() == 0);
                Self { data }
            }
        }

        impl<'a> ArgDataMethods for $name<'a> {
            fn type_(&self) -> Type {
                $nsi_type
            }

            fn len(&self) -> usize {
                self.data.len() / $nsi_type.elemensize()
            }

            fn as_c_ptr(&self) -> *const c_void {
                self.data.as_ptr() as _
            }
        }
    };
}

macro_rules! nsi_tuple_data_def {
    ($type: tt, $len: expr, $name: ident, $nsi_type: expr) => {
        #[derive(Debug)]
        pub struct $name<'a> {
            data: &'a [$type; $len],
        }

        impl<'a> $name<'a> {
            pub fn new(data: &'a [$type; $len]) -> Self {
                Self { data }
            }
        }

        impl<'a> ArgDataMethods for $name<'a> {
            fn type_(&self) -> Type {
                $nsi_type
            }

            fn len(&self) -> usize {
                1
            }

            fn as_c_ptr(&self) -> *const c_void {
                self.data.as_ptr() as _
            }
        }
    };
}

nsi_data_def!(f32, Float, Type::Float);
nsi_data_def!(f64, Double, Type::Double);
nsi_data_def!(i32, Integer, Type::Integer);

/// Reference type *with* lifetime guarantees.
///
/// Prefer this over using a raw [`Pointer`]
/// as it allows the compiler to check that
/// the data you reference outlives the
/// [`Context`](context::Context) you eventually
/// send it to.
///
/// This gets converted to a raw pointer when passed
/// through the FFI boundary.
/// ```
/// # use nsi_core as nsi;
/// struct Payload {
///     some_data: u32,
/// }
///
/// let ctx = nsi::Context::new(None).unwrap();
///
/// // Lots of scene setup omitted ...
///
/// // Setup a custom output driver and send
/// // a payload to it through the FFI boundary
/// ctx.create("driver", nsi::node::OUTPUT_DRIVER, None);
/// ctx.connect("driver", None, "beauty", "outputdrivers", None);
/// let payload = Payload { some_data: 42 };
/// ctx.set_attribute(
///     "driver",
///     &[
///         nsi::string!("drivername", "custom_driver"),
///         // Payload gets sent as raw pointer through
///         // the FFI boundary.
///         nsi::reference!("payload", Some(&payload)),
///     ],
/// );
///
/// // We need to explicitly call drop here as
/// // ctx's lifetime is pegged to that of payload.
/// drop(ctx);
/// ```
#[derive(Debug)]
pub struct Reference<'a> {
    data: *const c_void,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Reference<'a> {
    pub fn new<T: Sized>(data: Option<&'a T>) -> Self {
        Self {
            data: data
                .map(|p| p as *const _ as _)
                .unwrap_or(core::ptr::null()),
            _marker: PhantomData,
        }
    }
}

impl<'a> ArgDataMethods for Reference<'a> {
    fn type_(&self) -> Type {
        Type::Pointer
    }

    fn len(&self) -> usize {
        1
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data
    }
}

pub trait CallbackPtr {
    #[doc(hidden)]
    #[allow(clippy::wrong_self_convention)]
    fn to_ptr(self) -> *const c_void;
}

#[derive(Debug)]
pub struct Callback<'a> {
    data: *const c_void,
    _marker: PhantomData<&'a mut ()>,
}

impl<'a> Callback<'a> {
    pub fn new<T: CallbackPtr>(data: T) -> Self {
        Self {
            data: data.to_ptr(),
            _marker: PhantomData,
        }
    }
}

impl<'a> ArgDataMethods for Callback<'a> {
    fn type_(&self) -> Type {
        Type::Pointer
    }

    fn len(&self) -> usize {
        1
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data
    }
}

/// Raw pointer type *without* lifetime guarantees.
///
/// This can't guarantee that the data this points to
/// outlives the [`Context`](context::Context) you
/// eventually send this to. This is your responsibility.
///
/// If you need to send pointers a better alternative
/// is the [`Reference`] type that allows the compiler
/// to check that the the referenced data outlives the
/// [`Context`](context::Context).
#[derive(Debug)]
pub struct Pointer {
    data: *const c_void,
}

impl Pointer {
    /// # Safety
    /// This is marked unsafe because the responsibility
    /// to ensure the pointer can be safely de-referenced
    /// after the function has returned lies with the user.
    ///
    /// [`Reference`] is a *safe* alternative.
    pub unsafe fn new(data: *const c_void) -> Self {
        Self { data }
    }
}

impl ArgDataMethods for Pointer {
    fn type_(&self) -> Type {
        Type::Pointer
    }

    fn len(&self) -> usize {
        1
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data
    }
}

#[derive(Debug)]
pub struct String {
    #[allow(dead_code)]
    data: CString,
    // The FFI API needs a pointer to a C string
    pointer: *const c_void,
}

impl String {
    pub fn new<T: Into<Vec<u8>>>(data: T) -> Self {
        let data = CString::new(data).unwrap();
        let pointer = data.as_ptr() as _;

        String { data, pointer }
    }
}

impl ArgDataMethods for String {
    fn type_(&self) -> Type {
        Type::String
    }

    fn len(&self) -> usize {
        1
    }

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
        &self.pointer as *const *const std::ffi::c_void as _
    }
}

nsi_data_array_def!(f32, Floats, Type::Float);
nsi_data_array_def!(f64, Doubles, Type::Double);
nsi_data_array_def!(i32, Integers, Type::Integer);
nsi_data_array_def!(f32, Colors, Type::Color);
nsi_data_array_def!(f32, Points, Type::Point);
nsi_data_array_def!(f32, Vectors, Type::Vector);
nsi_data_array_def!(f32, Normals, Type::Normal);
nsi_data_array_def!(f32, Matrices, Type::Matrix);
nsi_data_array_def!(f64, DoubleMatrices, Type::DoubleMatrix);

/// Reference array type *with* lifetime guarantees.
///
/// Prefer this over using a raw [`Pointers`]
/// as it allows the compiler to check that
/// the data you reference outlives the
/// [`Context`](context::Context) you eventually send
/// it to.
///
/// This gets converted to an array of raw pointers when
/// passed through the FFI boundary.
#[derive(Debug)]
pub struct References<'a> {
    data: Vec<*const c_void>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> References<'a> {
    pub fn new<T>(data: &'a [Option<&'a T>]) -> Self {
        debug_assert!(data.len() % Type::Pointer.elemensize() == 0);

        let mut c_data = Vec::<*const c_void>::with_capacity(data.len());

        for e in data {
            c_data.push(
                e.map(|p| p as *const _ as *const c_void)
                    .unwrap_or(core::ptr::null()),
            );
        }

        Self {
            data: c_data,
            _marker: PhantomData,
        }
    }
}

impl<'a> ArgDataMethods for References<'a> {
    fn type_(&self) -> Type {
        Type::Pointer
    }

    fn len(&self) -> usize {
        self.data.len() / Type::Pointer.elemensize()
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data.as_ptr() as _
    }
}

/// Raw pointer array type *without* lifetime guarantees.
///
/// This can't guarantee that the data this points to
/// outlives the [`Context`](context::Context) you
/// eventually send this to. This is your responsibility.
///
/// If you need to send pointers a better alternative
/// is the [`References`] type that allows the compiler
/// to check that the the referenced data outlives the
/// [`Context`](context::Context).
#[derive(Debug)]
pub struct Pointers<'a> {
    data: &'a [*const c_void],
}

impl<'a> Pointers<'a> {
    /// # Safety
    /// This is marked unsafe because the responsibility
    /// to ensure the pointer can be safely de-referenced
    /// after the function has returned lies with the user.
    ///
    /// [`References`] is a *safe* alternative.
    pub unsafe fn new(data: &'a [*const c_void]) -> Self {
        Self { data }
    }
}

impl<'a> ArgDataMethods for Pointers<'a> {
    fn type_(&self) -> Type {
        Type::Pointer
    }

    fn len(&self) -> usize {
        self.data.len() / Type::Pointer.elemensize()
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data.as_ptr() as _
    }
}

#[derive(Debug)]
pub struct Strings {
    #[allow(dead_code)]
    data: Vec<CString>,
    pointer: Vec<*const c_void>,
}

impl Strings {
    pub fn new<T: Into<Vec<u8>> + Copy>(data: &[T]) -> Self {
        let data = data
            .iter()
            .map(|s| CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let pointer = data.iter().map(|s| s.as_ptr() as _).collect();

        Strings { data, pointer }
    }
}

impl ArgDataMethods for Strings {
    fn type_(&self) -> Type {
        Type::String
    }

    fn len(&self) -> usize {
        self.pointer.len()
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.pointer.as_ptr() as _
    }
}

nsi_tuple_data_def!(f32, 3, Color, Type::Color);
nsi_tuple_data_def!(f32, 3, Point, Type::Point);
nsi_tuple_data_def!(f32, 3, Vector, Type::Vector);
nsi_tuple_data_def!(f32, 3, Normal, Type::Normal);
nsi_tuple_data_def!(f32, 16, Matrix, Type::Matrix);
nsi_tuple_data_def!(f64, 16, DoubleMatrix, Type::DoubleMatrix);

/// Identifies an [`Arg`]’s data type.
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(i32)]
pub(crate) enum Type {
    /// A single [`f32`] value.
    Float = NSIType::Float as _,
    /// A single [`f64`] value.
    Double = NSIType::Double as _,
    /// Single [`i32`] value.
    Integer = NSIType::Integer as _,
    /// A [`String`].
    String = NSIType::String as _,
    /// Color, given as three [`f32`] values,
    /// usually in the range `0..1`. Red would e.g. be `[1.0, 0.0,
    /// 0.0]. Assumed to be in a linear color space.`
    Color = NSIType::Color as _,
    /// Point, given as three [`f32`] values.
    Point = NSIType::Point as _,
    /// Vector, given as three [`f32`] values.
    Vector = NSIType::Vector as _,
    /// Normal vector, given as three [`f32`] values.
    Normal = NSIType::Normal as _,
    /// Transformation matrix, given as 16 [`f32`] values.
    Matrix = NSIType::Matrix as _,
    /// Transformation matrix, given as 16 [`f64`] values.
    DoubleMatrix = NSIType::DoubleMatrix as _,
    /// Raw (`*const T`) pointer.
    Pointer = NSIType::Pointer as _,
}

impl Type {
    /// Returns the number of components of the resp. type.
    #[inline]
    pub(crate) fn elemensize(&self) -> usize {
        match self {
            Type::Float => 1,
            Type::Double => 1,
            Type::Integer => 1,
            Type::String => 1,
            Type::Color => 3,
            Type::Point => 3,
            Type::Vector => 3,
            Type::Normal => 3,
            Type::Matrix => 16,
            Type::DoubleMatrix => 16,
            Type::Pointer => 1,
        }
    }
}

/// Create a [`Float`] argument.
#[macro_export]
macro_rules! float {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Float::new($value)))
    };
}

/// Create a [`Float`] array argument.
#[macro_export]
macro_rules! floats {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Floats::new($value)))
    };
}

/// Create a [`Double`] precision argument.
#[macro_export]
macro_rules! double {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Double::new($value)))
    };
}

/// Create a [`Double`] precision array argument.
#[macro_export]
macro_rules! doubles {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Doubles::new($value)))
    };
}

/// Create a [`Integer`] argument.
#[macro_export]
macro_rules! integer {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Integer::new($value)))
    };
}

/// Create a [`Integer`] array argument.
#[macro_export]
macro_rules! integers {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Integers::new($value)))
    };
}

/// Create a [`Color`] argument.
#[macro_export]
macro_rules! color {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Color::new($value)))
    };
}

/// Create a [`Color`] array argument.
#[macro_export]
macro_rules! colors {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Colors::new($value)))
    };
}

/// Create a [`Point`] argument.
#[macro_export]
macro_rules! point {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Point::new($value)))
    };
}

/// Create a [`Point`] array argument.
#[macro_export]
macro_rules! points {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Points::new($value)))
    };
}

/// Create a [`Vector`] argument.
#[macro_export]
macro_rules! vector {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Vector::new($value)))
    };
}

/// Create a [`Vector`] array argument.
#[macro_export]
macro_rules! vectors {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Vectors::new($value)))
    };
}

/// Create a [`Normal`] argument.
#[macro_export]
macro_rules! normal {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Normal::new($value)))
    };
}

/// Create a [`Normal`] array argument.
#[macro_export]
macro_rules! normals {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Normals::new($value)))
    };
}

/// Create a [`Matrix`] row-major, 4×4 transformation matrix argument.
/// The matrix is given as 16 [`f32`] values.
#[macro_export]
macro_rules! matrix {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Matrix::new($value)))
    };
}

/// Create a [`Matrices`] row-major, 4×4 transformation matrices argument.
/// Each matrix is given as 16 [`f32`] values.
#[macro_export]
macro_rules! matrices {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Matrices::new($value)))
    };
}

/// Create a [`DoubleMatrix`] row-major, 4×4 transformation matrix argument.
/// The matrix is given as 16 [`f64`] values.
/// # Example
/// ```
/// # use nsi_core as nsi;
/// # let ctx = nsi::Context::new(None).unwrap();
/// // Setup a transform node.
/// ctx.create("xform", nsi::node::TRANSFORM, None);
/// ctx.connect("xform", None, ".root", "objects", None);
///
/// // Translate 5 units along z-axis.
/// ctx.set_attribute(
///     "xform",
///     &[nsi::double_matrix!(
///         "transformationmatrix",
///         &[1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,]
///     )],
/// );
/// ```
#[macro_export]
macro_rules! double_matrix {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::DoubleMatrix::new($value)))
    };
}

/// Create a [`DoubleMatrices`] row-major, 4×4 transformation matrices argument.
/// Each matrix is given as 16 [`f64`] values.
#[macro_export]
macro_rules! double_matrices {
    ($name: tt, $value: expr) => {
        nsi::Arg::new(
            $name,
            nsi::ArgData::from(nsi::DoubleMatrices::new($value)),
        )
    };
}

/// Create a [`String`] argument.
/// # Example
/// ```
/// # use nsi_core as nsi;
/// // Create rendering context.
/// let ctx =
///     nsi::Context::new(Some(&[nsi::string!("streamfilename", "stdout")]))
///         .expect("Could not create NSI context.");
/// ```
#[macro_export]
macro_rules! string {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::String::new($value)))
    };
}

/// Create a [`String`] array argument.
/// # Example
/// ```
/// # use nsi_core as nsi;
/// # let ctx = nsi::Context::new(None).unwrap();
/// // One of these is not an actor:
/// ctx.set_attribute(
///     "dummy",
///     &[nsi::strings!(
///         "actors",
///         &["Klaus Kinski", "Giorgio Moroder", "Rainer Brandt"]
///     )],
/// );
/// ```
#[macro_export]
macro_rules! strings {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Strings::new($value)))
    };
}

/// Create a [`Reference`] argument.
#[macro_export]
macro_rules! reference {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Reference::new($value)))
    };
}

/// Create a [`Reference`] array argument.
#[macro_export]
macro_rules! references {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::References::new($value)))
    };
}

/// Create a [`Callback`] argument.
#[macro_export]
macro_rules! callback {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Callback::new($value)))
    };
}

/// Create a [`Pointer`] array argument.
#[macro_export]
macro_rules! pointer {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Pointer::new($value)))
    };
}

/// Create a [`Pointer`] array argument.
#[macro_export]
macro_rules! pointers {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Pointers::new($value)))
    };
}
