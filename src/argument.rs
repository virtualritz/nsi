use enum_dispatch::enum_dispatch;
use nsi_sys::*;
use std::{ffi::CString, marker::PhantomData};

// Needed for docs.
#[allow(unused_imports)]
use crate::*;

#[inline]
pub(crate) fn get_c_param_vec(args_in: &ArgSlice) -> (i32, *const NSIParam_t, Vec<NSIParam_t>) {
    let mut args_out = Vec::<NSIParam_t>::with_capacity(args_in.len());
    for arg_in in args_in {
        args_out.push(NSIParam_t {
            name: arg_in.name.as_ptr(),
            data: arg_in.data.as_c_ptr(),
            type_: arg_in.data.type_() as i32,
            arraylength: arg_in.array_length as i32,
            count: (arg_in.data.len() / arg_in.array_length) as u64,
            flags: arg_in.flags as std::os::raw::c_int,
        });
    }
    (
        args_out.len() as i32,
        args_out.as_ptr() as *const NSIParam_t,
        args_out,
    )
}

/// A slice of (optional) arguments passed to a method of [`Context`].
pub type ArgSlice<'a, 'b> = [Arg<'a, 'b>];

/// A vector of (optional) arguments passed to a method of [`Context`].
pub type ArgVec<'a, 'b> = Vec<Arg<'a, 'b>>;

/// An (optional) argument passed to a method of [`Context`].
pub struct Arg<'a, 'b> {
    pub(crate) name: CString,
    pub(crate) data: ArgData<'a, 'b>,
    // length of each element if an array type
    pub(crate) array_length: usize,
    // number of elements
    pub(crate) flags: u32,
}

impl<'a, 'b> Arg<'a, 'b> {
    #[inline]
    pub fn new(name: &str, data: ArgData<'a, 'b>) -> Self {
        Arg {
            name: CString::new(name).unwrap(),
            data,
            array_length: 1,
            flags: 0,
        }
    }

    /// Sets the length of the argument for each element.
    #[inline]
    pub fn array_len(mut self, length: usize) -> Self {
        self.array_length = length;
        self.flags |= NSIParamIsArray;
        self
    }

    /// Marks this argument as having per-face granularity.
    #[inline]
    pub fn per_face(mut self) -> Self {
        self.flags |= NSIParamPerFace;
        self
    }

    /// Marks this argument as having per-vertex granularity.
    #[inline]
    pub fn per_vertex(mut self) -> Self {
        self.flags |= NSIParamPerVertex;
        self
    }

    /// Marks this argument as to be interpolated linearly.
    #[inline]
    pub fn linear_interpolation(mut self) -> Self {
        self.flags |= NSIParamInterpolateLinear;
        self
    }
}

#[enum_dispatch(ArgData)]
pub(crate) trait ArgDataMethods {
    //const TYPE: Type;
    fn type_(&self) -> Type;
    fn len(&self) -> usize;
    fn as_c_ptr(&self) -> *const std::ffi::c_void;
}

/// A variant describing data passed to the renderer.
///
/// # Lifetimes
/// Lifetime `'a` is for any tuple or array type as these are
/// passed as references and only need to live as long as the
/// function call where they get passed.
///
/// Lifetime `'b` is for the arbitrary reference type. This is
/// pegged to the lifetime of the [`Context`]. Use this to
/// pass arbitray Rust data through the FFI boundary.
#[enum_dispatch]
pub enum ArgData<'a, 'b> {
    /// Single [`f32`) value.
    Float,
    Floats(Floats<'a>),
    /// Single [`f64`] value.
    Double,
    Doubles(Doubles<'a>),
    /// Single [`i32`] value.
    Integer,
    /// An [[`i32`]] array.
    Integers(Integers<'a>),
    /// Single [`i32`] value.
    Unsigned,
    /// An [[`i32`]] array.
    Unsigneds(Unsigneds<'a>),
    /// A [`String`].
    String(String),
    /// A [[`String`]] array.
    Strings(Strings),
    /// Color in linear space, given as a red, green, blue triplet
    /// of [`f32`] values; usually in the range `0..1`.
    Color(Color<'a>),
    /// An arry of colors.
    Colors(Colors<'a>),
    /// Point, given as three [`f32`] values.
    Point(Point<'a>),
    Points(Points<'a>),
    /// Vector, given as three [`f32`] values.
    Vector(Vector<'a>),
    Vectors(Vectors<'a>),
    /// Normal vector, given as three [`f32`] values.
    /// values.
    Normal(Normal<'a>),
    Normasl(Normals<'a>),
    /// Transformation matrix, given as 16 [`f32`] floating
    /// point datas.
    Matrix(Matrix<'a>),
    Matrices(Matrices<'a>),
    /// Transformation matrix, given as 16 [`f64`] floating
    /// point datas.
    DoubleMatrix(DoubleMatrix<'a>),
    DoubleMatrices(DoubleMatrices<'a>),
    /// Reference to arbitrary data.
    Reference(Reference<'b>),
    References(References<'b>),
    /// Raw (`*const T`) pointer.
    Pointer,
    Pointers(Pointers<'a>),
}

macro_rules! nsi_data_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
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

            fn as_c_ptr(&self) -> *const std::ffi::c_void {
                &self.data as *const $type as _
            }
        }
    };
}

macro_rules! nsi_data_array_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
        pub struct $name<'a> {
            data: &'a [$type],
        }

        impl<'a> $name<'a> {
            pub fn new(data: &'a [$type]) -> Self {
                debug_assert!(data.len() % $nsi_type.element_size() == 0);
                Self { data }
            }
        }

        impl<'a> ArgDataMethods for $name<'a> {
            fn type_(&self) -> Type {
                $nsi_type
            }

            fn len(&self) -> usize {
                self.data.len() / $nsi_type.element_size()
            }

            fn as_c_ptr(&self) -> *const std::ffi::c_void {
                self.data.as_ptr() as _
            }
        }
    };
}

macro_rules! nsi_tuple_data_def {
    ($type: tt, $len: expr, $name: ident, $nsi_type: expr) => {
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

            fn as_c_ptr(&self) -> *const std::ffi::c_void {
                self.data.as_ptr() as _
            }
        }
    };
}

nsi_data_def!(f32, Float, Type::Float);
nsi_data_def!(f64, Double, Type::Double);
nsi_data_def!(i32, Integer, Type::Integer);
nsi_data_def!(u32, Unsigned, Type::Integer);
//nsi_data_def!(*const std::ffi::c_void, Pointer, Type::Pointer);

/// Reference type *with* lifetime guaratees.
///
/// Prefer this over using a raw [`Pointer`]
/// as it allows the compiler to check that
/// the data you reference outlives the
/// [`Context`] you eventually send it to.
///
/// This gets converted to a raw pointer when passed
/// through the FFI boundary.
/// ```
/// struct Payload {
///     some_data: u32,
/// }
///
/// let ctx = nsi::Context::new(&[]).unwrap();
///
/// // Lots of scene setup omitted ...
///
/// // Setup a custom output driver and send
/// // a payload to it through the FFI boundary
/// ctx.create("driver", nsi::NodeType::OutputDriver, &[]);
/// ctx.connect("driver", "", "beauty", "outputdrivers", &[]);
/// let payload = Payload {
///     some_data: 42,
/// };
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
pub struct Reference<'a> {
    data: *const std::ffi::c_void,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Reference<'a> {
    pub fn new<T>(data: Option<&'a T>) -> Self {
        Self {
            data: data
                .map(|p| p as *const _ as *const std::ffi::c_void)
                .unwrap_or(std::ptr::null()),
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

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
        self.data
    }
}

/// Raw pointer type *without* lifietime guaratees.
///
/// This can't guarantee that the data this
/// points to outlives the [`Context`] you
/// eventually send this to. This is the user's
/// responsibility.
///
/// If you need to send a pointer a better
/// alternative is the [`Reference`] type
/// that allows the compiler to check that
/// the the data outlives the [`Context`].
pub struct Pointer {
    data: *const std::ffi::c_void,
}

impl Pointer {
    pub unsafe fn new(data: *const std::ffi::c_void) -> Self {
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

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
        self.data
    }
}

pub struct String {
    _data: CString,
    // The FFI API needs a pointer to a C string
    pointer: *const std::ffi::c_void,
}

impl String {
    pub fn new<T: Into<Vec<u8>>>(data: T) -> Self {
        let _data = CString::new(data).unwrap();
        let pointer = _data.as_ptr() as _;

        String { _data, pointer }
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
        unsafe { std::mem::transmute(&self.pointer) }
    }
}

nsi_data_array_def!(f32, Floats, Type::Float);
nsi_data_array_def!(f64, Doubles, Type::Double);
nsi_data_array_def!(i32, Integers, Type::Integer);
nsi_data_array_def!(u32, Unsigneds, Type::Integer);
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
/// [`Context`] you eventually send it to.
///
/// This gets converted to an array of raw pointers when
/// passed through the FFI boundary.
pub struct References<'a> {
    data: Vec<*const std::ffi::c_void>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> References<'a> {
    pub fn new<T>(data: &'a [Option<&'a T>]) -> Self {
        debug_assert!(data.len() % Type::Pointer.element_size() == 0);

        let mut c_data = Vec::<*const std::ffi::c_void>::with_capacity(data.len());

        for e in data {
            c_data.push(
                e.map(|p| p as *const _ as *const std::ffi::c_void)
                    .unwrap_or(std::ptr::null()),
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
        self.data.len() / Type::Pointer.element_size()
    }

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
        self.data.as_ptr() as _
    }
}

/// Raw pointer array type *without* lifetime guaratees.
///
/// This can't guarantee that the data this points to
/// outlives the [`Context`] you eventually send this
/// to. This is your responsibility.
///
/// If you need to send pointers a better alternative
/// is the [`References`] type that allows the compiler
/// to check that the the referenced data outlives the
/// [`Context`].
pub struct Pointers<'a> {
    data: &'a [*const std::ffi::c_void],
}

impl<'a> Pointers<'a> {
    /// This is marked unsafe because the responsibility
    /// to ensure the pointer can be safely dereferenced
    /// after the function has returned lies with the user.
    ///
    /// [`References`] is a *safe* alternative.
    pub unsafe fn new(data: &'a [*const std::ffi::c_void]) -> Self {
        Self { data }
    }
}

impl<'a> ArgDataMethods for Pointers<'a> {
    fn type_(&self) -> Type {
        Type::Pointer
    }

    fn len(&self) -> usize {
        self.data.len() / Type::Pointer.element_size()
    }

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
        self.data.as_ptr() as _
    }
}

pub struct Strings {
    _data: Vec<CString>,
    pointer: Vec<*const std::ffi::c_void>,
}

impl Strings {
    pub fn new<T: Into<Vec<u8>> + Copy>(data: &[T]) -> Self {
        let mut _data = Vec::<CString>::with_capacity(data.len());
        let mut pointer = Vec::<*const std::ffi::c_void>::with_capacity(data.len());

        data.iter().for_each(|s| {
            _data.push(CString::new(*s).unwrap());
            pointer.push(_data.last().unwrap().as_ptr() as _);
        });

        Strings { _data, pointer }
    }
}

impl ArgDataMethods for Strings {
    fn type_(&self) -> Type {
        Type::String
    }

    fn len(&self) -> usize {
        self.pointer.len()
    }

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
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
    /// A single 32-bit ([`f32`]) floating point value.
    Float = NSIType_t_NSITypeFloat as i32,
    /// A single 64-bit ([`f64`]) floating point value.
    Double = NSIType_t_NSITypeDouble as i32,
    /// Single 32-bit ([`i32`]) integer data.
    Integer = NSIType_t_NSITypeInteger as i32,
    /// A [`String`].
    String = NSIType_t_NSITypeString as i32,
    /// Color, given as three 32-bit ([`i32`]) floating point datas,
    /// usually in the range `0..1`. Red would e.g. be `[1.0, 0.0,
    /// 0.0]`
    Color = NSIType_t_NSITypeColor as i32,
    /// Point, given as three 32-bit ([`f32`])floating point datas.
    Point = NSIType_t_NSITypePoint as i32,
    /// Vector, given as three 32-bit ([`f32`]) floating point datas.
    Vector = NSIType_t_NSITypeVector as i32,
    /// Normal vector, given as three 32-bit ([`f32`]) floating point
    /// datas.
    Normal = NSIType_t_NSITypeNormal as i32,
    /// Transformation matrix, given as 16 32-bit ([`f32`]) floating
    /// point datas.
    Matrix = NSIType_t_NSITypeMatrix as i32,
    /// Transformation matrix, given as 16 64-bit ([`f64`]) floating
    /// point datas.
    DoubleMatrix = NSIType_t_NSITypeDoubleMatrix as i32,
    /// Raw (`*const T`) pointer.
    Pointer = NSIType_t_NSITypePointer as i32,
}

impl Type {
    /// Returns the size of the resp. type in bytes.
    #[inline]
    pub(crate) fn element_size(&self) -> usize {
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

#[macro_export]
macro_rules! float {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Float::new($value)))
    };
}

#[macro_export]
macro_rules! floats {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Floats::new($value)))
    };
}

#[macro_export]
macro_rules! double {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Double::new($value)))
    };
}

#[macro_export]
macro_rules! doubles {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Doubles::new($value)))
    };
}

#[macro_export]
macro_rules! integer {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Integer::new($value)))
    };
}

#[macro_export]
macro_rules! integers {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Integers::new($value)))
    };
}

#[macro_export]
macro_rules! unsigned {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Unsigned::new($value)))
    };
}

#[macro_export]
macro_rules! unsigneds {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Unsigneds::new($value)))
    };
}

#[macro_export]
macro_rules! color {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Color::new($value)))
    };
}

#[macro_export]
macro_rules! colors {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Colors::new($value)))
    };
}

#[macro_export]
macro_rules! point {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Point::new($value)))
    };
}

#[macro_export]
macro_rules! points {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Points::new($value)))
    };
}

#[macro_export]
macro_rules! vector {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Vector::new($value)))
    };
}

#[macro_export]
macro_rules! vectors {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Vectors::new($value)))
    };
}

#[macro_export]
macro_rules! normal {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Normal::new($value)))
    };
}

#[macro_export]
macro_rules! normals {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Normals::new($value)))
    };
}

#[macro_export]
macro_rules! matrix {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Matrix::new($value)))
    };
}

#[macro_export]
macro_rules! matrices {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Matrices::new($value)))
    };
}

/// A macro to create a double precision 4×4 matrix argument.
/// # Example
/// ```
/// // create rendering context.
/// let ctx = nsi::Context::new(&[]).unwrap();
///
/// // Setup a transform node.
/// ctx.create("xform", nsi::NodeType::Transform, &[]);
/// ctx.connect("xform", "", ".root", "objects", &[]);
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

/// A macro to create a [`DoubleMatrices`] double precision 4×4 matrix
/// array argument.
#[macro_export]
macro_rules! double_matrices {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::DoubleMatrices::new($value)))
    };
}

/// A macro to create a [`String`] argument.
/// # Example
/// ```
/// // Create rendering context.
/// let ctx = nsi::Context::new(&[nsi::string!(
///     "streamfilename",
///     "stdout"
/// )])
/// .expect("Could not create NSI context.");
/// ```
#[macro_export]
macro_rules! string {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::String::new($value)))
    };
}

/// A macro to create a string array argument.
/// # Example
/// ```
/// // Create rendering context.
/// let ctx = nsi::Context::new(&[]).unwrap();
/// // One of these is not an actor:
/// ctx.create("dummy", nsi::NodeType::Attributes, &[
///    nsi::strings!("actors", &["Klaus Kinski", "Giorgio Moroder", "Rainer Brandt", "Helge Schneider"]).array_len(2)
/// ]);
/// ```
#[macro_export]
macro_rules! strings {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Strings::new($value)))
    };
}

#[macro_export]
macro_rules! reference {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Reference::new($value)))
    };
}

#[macro_export]
macro_rules! references {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::References::new($value)))
    };
}

#[macro_export]
macro_rules! pointer {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Pointer::new($value)))
    };
}

#[macro_export]
macro_rules! pointers {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Pointers::new($value)))
    };
}
