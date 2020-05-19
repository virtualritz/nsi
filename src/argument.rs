use crate::*;
use enum_dispatch::enum_dispatch;
use nsi_sys;
use std::ffi::CString;

#[inline]
pub(crate) fn get_c_param_vec(
    args_in: &ArgVec,
) -> (Vec<nsi_sys::NSIParam_t>, Vec<*const std::ffi::c_void>) {
    // we create an array that holds our *const string pointers
    // so we can take *const *const strings of these before sending
    // to the FFI
    let mut string_ptr_vec = Vec::<*const _>::new();

    let mut args_out = Vec::<nsi_sys::NSIParam_t>::with_capacity(args_in.len());
    for arg_in in args_in {
        args_out.push({
            let data_ptr = if Type::String == arg_in.data.type_() {
                string_ptr_vec.push(arg_in.data.as_c_ptr());
                unsafe {
                    std::mem::transmute::<*const *const _, *const _>(string_ptr_vec.last().unwrap())
                }
            } else {
                arg_in.data.as_c_ptr()
            };

            let param = nsi_sys::NSIParam_t {
                name: arg_in.name.as_ptr(),
                data: data_ptr,
                type_: arg_in.data.type_() as i32,
                arraylength: arg_in.array_length as i32,
                count: (arg_in.data.len() / arg_in.array_length) as u64,
                flags: arg_in.flags as std::os::raw::c_int,
            };

            param
        });
    }

    (args_out, string_ptr_vec)
}

/// A vector of (optional) [`Context`] method arguments.
pub type ArgVec<'a> = Vec<Arg<'a>>;

pub struct Arg<'a> {
    pub(crate) name: CString,
    pub(crate) data: Data<'a>,
    // length of each element if an array type
    pub(crate) array_length: usize,
    // number of elements
    pub(crate) flags: u32,
}

impl<'a> Arg<'a> {
    #[inline]
    pub fn new(name: &str, data: Data<'a>) -> Self {
        Arg {
            name: CString::new(name).unwrap(),
            data: data,
            array_length: 1,
            flags: 0,
        }
    }

    #[inline]
    pub fn array_len(mut self, length: usize) -> Self {
        self.array_length = length;
        self.flags |= nsi_sys::NSIParamIsArray;
        self
    }

    #[inline]
    pub fn per_face(mut self) -> Self {
        self.flags |= nsi_sys::NSIParamPerFace;
        self
    }

    #[inline]
    pub fn per_vertex(mut self) -> Self {
        self.flags |= nsi_sys::NSIParamPerVertex;
        self
    }

    #[inline]
    pub fn linear_interpolation(mut self) -> Self {
        self.flags |= nsi_sys::NSIParamInterpolateLinear;
        self
    }
}

#[enum_dispatch(Data)]
trait DataMethods {
    //const TYPE: Type;
    fn type_(&self) -> Type;
    fn len(&self) -> usize;
    fn as_c_ptr(&self) -> *const std::ffi::c_void;
}

#[enum_dispatch]

pub enum Data<'a> {
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

        impl DataMethods for $name {
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

        impl<'a> DataMethods for $name<'a> {
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

        impl<'a> DataMethods for $name<'a> {
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
nsi_data_def!(*const std::ffi::c_void, Pointer, Type::Pointer);

nsi_data_array_def!(f32, Floats, Type::Float);
nsi_data_array_def!(f64, Doubles, Type::Double);
nsi_data_array_def!(i32, Integers, Type::Integer);
nsi_data_array_def!(u32, Unsigneds, Type::Integer);
nsi_data_array_def!(*const std::ffi::c_void, Pointers, Type::Pointer);
nsi_data_array_def!(f32, Colors, Type::Color);
nsi_data_array_def!(f32, Points, Type::Point);
nsi_data_array_def!(f32, Vectors, Type::Vector);
nsi_data_array_def!(f32, Normals, Type::Normal);
nsi_data_array_def!(f32, Matrices, Type::Matrix);
nsi_data_array_def!(f64, DoubleMatrices, Type::DoubleMatrix);

pub struct String {
    data: CString,
}

impl String {
    pub fn new<T: Into<Vec<u8>>>(data: T) -> Self {
        String {
            data: CString::new(data).unwrap(),
        }
    }
}

impl DataMethods for String {
    fn type_(&self) -> Type {
        Type::String
    }

    fn len(&self) -> usize {
        1
    }

    fn as_c_ptr(&self) -> *const std::ffi::c_void {
        self.data.as_ptr() as _
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
enum Type {
    /// A single 32-bit ([`f32`]) floating point value.
    Float = nsi_sys::NSIType_t_NSITypeFloat as i32,
    /// A single 64-bit ([`f64`]) floating point value.
    Double = nsi_sys::NSIType_t_NSITypeDouble as i32,
    /// Single 32-bit ([`i32`]) integer data.
    Integer = nsi_sys::NSIType_t_NSITypeInteger as i32,
    /// A [`String`].
    String = nsi_sys::NSIType_t_NSITypeString as i32,
    /// Color, given as three 32-bit ([`i32`]) floating point datas,
    /// usually in the range `0..1`. Red would e.g. be `[1.0, 0.0,
    /// 0.0]`
    Color = nsi_sys::NSIType_t_NSITypeColor as i32,
    /// Point, given as three 32-bit ([`f32`])floating point datas.
    Point = nsi_sys::NSIType_t_NSITypePoint as i32,
    /// Vector, given as three 32-bit ([`f32`]) floating point datas.
    Vector = nsi_sys::NSIType_t_NSITypeVector as i32,
    /// Normal vector, given as three 32-bit ([`f32`]) floating point
    /// datas.
    Normal = nsi_sys::NSIType_t_NSITypeNormal as i32,
    /// Transformation matrix, given as 16 32-bit ([`f32`]) floating
    /// point datas.
    Matrix = nsi_sys::NSIType_t_NSITypeMatrix as i32,
    /// Transformation matrix, given as 16 64-bit ([`f64`]) floating
    /// point datas.
    DoubleMatrix = nsi_sys::NSIType_t_NSITypeDoubleMatrix as i32,
    /// Raw (`*const T`) pointer.
    Pointer = nsi_sys::NSIType_t_NSITypePointer as i32,
}

impl Type {
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

/// A macro to specify an empty [`ArgVec`] to a [`Context`] method
/// that supports optional arguments.
///
/// ```
/// // Create rendering context.
/// let ctx = nsi::Context::new(nsi::no_arg!()).unwrap();
/// ```
#[macro_export]
macro_rules! no_arg {
    () => {
        &nsi::ArgVec::new()
    };
}

/// A macro to create an Argument aka: [`Arg::new()`].
///
/// ```
/// // Create rendering context.
/// let ctx = nsi::Context::new(&vec![nsi::arg!(
///     "streamfilename",
///     nsi::string!("stdout")
/// )])
/// .expect("Could not create NSI context.");
/// ```
#[macro_export]
macro_rules! arg {
    ($token:expr, $value:expr) => {
        nsi::Arg::new($token, $value)
    };
}

#[macro_export]
macro_rules! float {
    ($value: expr) => {
        nsi::Data::from(nsi::Float::new($value))
    };
}

#[macro_export]
macro_rules! floats {
    ($value: expr) => {
        nsi::Data::from(nsi::Floats::new($value))
    };
}

#[macro_export]
macro_rules! double {
    ($value: expr) => {
        nsi::Data::from(nsi::Double::new($value))
    };
}

#[macro_export]
macro_rules! doubles {
    ($value: expr) => {
        nsi::Data::from(nsi::Doubles::new($value))
    };
}

#[macro_export]
macro_rules! integer {
    ($value: expr) => {
        nsi::Data::from(nsi::Integer::new($value))
    };
}

#[macro_export]
macro_rules! integers {
    ($value: expr) => {
        nsi::Data::from(nsi::Integers::new($value))
    };
}

#[macro_export]
macro_rules! unsigned {
    ($value: expr) => {
        nsi::Data::from(nsi::Unsigned::new($value))
    };
}

#[macro_export]
macro_rules! unsigneds {
    ($value: expr) => {
        nsi::Data::from(nsi::Unsigneds::new($value))
    };
}

#[macro_export]
macro_rules! color {
    ($value: expr) => {
        nsi::Data::from(nsi::Color::new($value))
    };
}

#[macro_export]
macro_rules! colors {
    ($value: expr) => {
        nsi::Data::from(nsi::Colors::new($value))
    };
}

#[macro_export]
macro_rules! point {
    ($value: expr) => {
        nsi::Data::from(nsi::Point::new($value))
    };
}

#[macro_export]
macro_rules! points {
    ($value: expr) => {
        nsi::Data::from(nsi::Points::new($value))
    };
}

#[macro_export]
macro_rules! vector {
    ($value: expr) => {
        nsi::Data::from(nsi::Vector::new($value))
    };
}

#[macro_export]
macro_rules! vectors {
    ($value: expr) => {
        nsi::Data::from(nsi::Vector::new($value))
    };
}

#[macro_export]
macro_rules! normal {
    ($value: expr) => {
        nsi::Data::from(nsi::Normal::new($value))
    };
}

#[macro_export]
macro_rules! normals {
    ($value: expr) => {
        nsi::Data::from(nsi::Normals::new($value))
    };
}

#[macro_export]
macro_rules! matrix {
    ($value: expr) => {
        nsi::Data::from(nsi::Matrix::new($value))
    };
}

#[macro_export]
macro_rules! matrices {
    ($value: expr) => {
        nsi::Data::from(nsi::Matrices::new($value))
    };
}

#[macro_export]
macro_rules! double_matrix {
    ($value: expr) => {
        nsi::Data::from(nsi::DoubleMatrix::new($value))
    };
}

#[macro_export]
macro_rules! double_matrices {
    ($value: expr) => {
        nsi::Data::from(nsi::DoubleMatrices::new($value))
    };
}

#[macro_export]
macro_rules! string {
    ($value: expr) => {
        nsi::Data::from(nsi::String::new($value))
    };
}

/* FIXME
#[macro_export]
macro_rules! strings {
    ($value: expr) => {
        nsi::Data::from(nsi::Strings::new($value))
    };
}
*/

#[macro_export]
macro_rules! pointer {
    ($value: expr) => {
        nsi::Data::from(nsi::Pointer::new($value))
    };
}

#[macro_export]
macro_rules! pointers {
    ($value: expr) => {
        nsi::Data::from(nsi::Pointers::new($value))
    };
}
