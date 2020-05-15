use nsi_sys;
use std::{ffi::CString, os::raw::c_char};

//#[derive(Copy, Clone, Debug)]

macro_rules! to_nsi {
    ($self:ident, $string_ptr_vec:ident) => {{
        // Check that we have a type
        debug_assert!(Type::Invalid != $self.data.type_());
        debug_assert!($self.data.len() / $self.data.TYPE.element_size() / $self.array_length > 0);
        debug_assert!(
            ($self.data.len() / $self.data.TYPE.element_size()) % $self.array_length == 0
        );

        let data_ptr = match $self.data {
            Data::String(s) => {
                $string_ptr_vec.push(s.as_ptr());
                unsafe {
                    std::mem::transmute::<*const *const _, *const _>(
                        &(*$string_ptr_vec.last().unwrap()),
                    )
                }
            }
            Data::Strings(s) => {
                // FIXME
                panic!()
            }
            _ => $self.data.as_ptr(),
        };

        nsi_sys::NSIParam_t {
            name: $self.name.as_ptr(),
            data: data_ptr,
            type_: $self.data.TYPE as i32,
            arraylength: $self.array_length as i32,
            count: ($self.data.len() / $self.data.TYPE.element_size() / $self.array_length) as u64,
            flags: $self.flags as std::os::raw::c_int,
        }
    }};
}

macro_rules! get_c_param_vec {
    ( $args_in:ident, $args_out: expr ) => {
        /* $args_out =
            $args_in.iter().map(|arg| to_nsi!(arg, args_in);).collect::<Vec<_>>();
        }*/
        // we create an array that holds our *const string pointers
        // so we can take *const *const strings of these before sending
        // to the FFI
        let mut string_ptr_vec = Vec::<*const _>::new();
        for arg_in in $args_in {
            let tmp = to_nsi!(arg_in, string_ptr_vec);
            $args_out.push(tmp);
        }
    };
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
    //const NO_DATA: Data = Data::None;
    pub fn new(name: &str) -> Self {
        Arg {
            name: CString::new(name).unwrap(),
            data: Data::None,
            array_length: 0,
            flags: 0,
        }
    }

    pub fn with_data(&'a mut self, data: Data<'a>) -> &'a mut Self {
        self.data = data;
        self
    }

    pub fn with_array_length(&'a mut self, length: usize) -> &'a mut Self {
        self.array_length = length;
        self.flags |= nsi_sys::NSIParamIsArray;
        self
    }

    pub fn with_per_face(&'a mut self) -> &'a mut Self {
        self.flags |= nsi_sys::NSIParamPerFace;
        self
    }

    pub fn with_per_vertex(&'a mut self) -> &'a mut Self {
        self.flags |= nsi_sys::NSIParamPerVertex;
        self
    }

    pub fn with_linear_interpolation(&'a mut self) -> &'a mut Self {
        self.flags |= nsi_sys::NSIParamInterpolateLinear;
        self
    }
}

trait DataMethods<T> {
    fn type_() -> Type;
    fn len(&self) -> usize;
    fn as_ptr(&self) -> *const T;
}

#[macro_export]
macro_rules! nsi_data_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
        struct $name {
            data: $type,
        }

        impl $name {
            pub fn new(data: $type) -> Self {
                Self { data }
            }
        }

        impl DataMethods for $name {
            pub(crate) fn type_() {
                $nsi_type
            }

            pub(crate) fn len(&self) -> usize {
                1
            }

            pub(crate) fn as_ptr(&self) -> *const $type {
                &self.data
            }
        }
    };
}

#[macro_export]
macro_rules! nsi_datas_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
        struct $name<'a> {
            data: &'a [$type],
        }

        impl<'a> $name<'a> {
            const TYPE: Type = $nsi_type;

            pub fn new(data: &'a [$type]) -> Self {
                debug_assert!(data.len() % $nsi_type.element_size() == 0);
                Self { data }
            }

            pub(crate) fn len(&self) -> usize {
                self.data.len() / $nsi_type.element_size()
            }

            pub(crate) fn as_ptr(&self) -> *const $type {
                self.data.as_ptr()
            }
        }
    };
}

#[macro_export]
macro_rules! nsi_tuple_data_def {
    ($type: tt, $len: expr, $name: ident, $nsi_type: expr) => {
        struct $name<'a> {
            data: &'a [$type; $len],
        }

        impl<'a> $name<'a> {
            const TYPE: Type = $nsi_type;

            pub fn new(data: &'a [$type; $len]) -> Self {
                Self { data }
            }

            pub(crate) fn len(&self) -> usize {
                1
            }

            pub(crate) fn as_ptr(&self) -> *const $type {
                self.data.as_ptr()
            }
        }
    };
}

nsi_data_def!(f32, Float, Type::Float);
nsi_data_def!(f64, Double, Type::Double);
nsi_data_def!(i32, Integer, Type::Integer);
nsi_data_def!(*const std::ffi::c_void, Pointer, Type::Pointer);

nsi_datas_def!(f32, Floats, Type::Float);
nsi_datas_def!(f64, Doubles, Type::Double);
nsi_datas_def!(i32, Integers, Type::Integer);
nsi_datas_def!(*const std::ffi::c_void, Pointers, Type::Pointer);
nsi_datas_def!(f32, Colors, Type::Color);
nsi_datas_def!(f32, Points, Type::Point);
nsi_datas_def!(f32, Vectors, Type::Vector);
nsi_datas_def!(f32, Normals, Type::Normal);
nsi_datas_def!(f32, Matrices, Type::Matrix);
nsi_datas_def!(f64, DoubleMatrices, Type::DoubleMatrix);

struct String {
    data: CString,
}

impl String {
    pub fn new<T: Into<Vec<u8>>>(data: T) -> Self {
        String {
            data: CString::new(data).unwrap(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        1
    }

    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.data.as_ptr()
    }
}

nsi_tuple_data_def!(f32, 3, Color, Type::Color);
nsi_tuple_data_def!(f32, 3, Point, Type::Point);
nsi_tuple_data_def!(f32, 3, Vector, Type::Vector);
nsi_tuple_data_def!(f32, 3, Normal, Type::Normal);
nsi_tuple_data_def!(f32, 16, Matrix, Type::Matrix);
nsi_tuple_data_def!(f64, 16, DoubleMatrix, Type::DoubleMatrix);

pub enum Data<'a> {
    None,
    /// Single 32-bit ([`f32`]) floating point data.
    Float(Float),
    Floats(Floats<'a>),
    /// Single 64-bit ([`f64`]) floating point data.
    Double(Double),
    Doubles(Doubles<'a>),
    /// Single 32-bit ([`i32`]) integer data.
    Integer(Integer),
    Integers(Integers<'a>),
    /// A [`String`].
    String(String),
    /// Color in linear space, given as three RGB 32-bit ([`f32`])
    /// floating point datas, usually in the range `0..1`.
    Color(Color<'a>),
    Colors(Colors<'a>),
    /// Point, given as three 32-bit ([`f32`])floating point datas.
    Point(Point<'a>),
    Points(Points<'a>),
    /// Vector, given as three 32-bit ([`f32`]) floating point datas.
    Vector(Vector<'a>),
    Vectors(Vectors<'a>),
    /// Normal vector, given as three 32-bit ([`f32`]) floating point
    /// datas.
    Normal(Normal<'a>),
    Normasl(Normals<'a>),
    /// Transformation matrix, given as 16 32-bit ([`f32`]) floating
    /// point datas.
    Matrix(Matrix<'a>),
    Matrices(Matrices<'a>),
    /// Transformation matrix, given as 16 64-bit ([`f64`]) floating
    /// point datas.
    DoubleMatrix(DoubleMatrix<'a>),
    DoubleMatrices(DoubleMatrices<'a>),
    /// Raw (`*const T`) pointer.
    Pointer(Pointer),
    Pointers(Pointers<'a>),
}

/// Identifies an [`Arg`]’s data type.
#[derive(Copy, Clone, Debug)]
pub enum Type {
    /// Undefined type.
    Invalid = 0, // nsi_sys::NSIType_t::NSITypeInvalid,
    /// Single 32-bit ([`f32`]) floating point data.
    Float = 1, // nsi_sys::NSIType_t::NSITypeFloat,
    /// Single 64-bit ([`f64`]) floating point data.
    Double = 1 | 0x10, // nsi_sys::NSIType_t::NSITypeFloat | 0x10,
    /// Single 32-bit ([`i32`]) integer data.
    Integer = 2, // nsi_sys::NSIType_t::NSITypeInteger,
    /// A [`String`].
    String = 3, // nsi_sys::NSIType_t::NSITypeString,
    /// Color, given as three 32-bit ([`i32`]) floating point datas,
    /// usually in the range `0..1`. Red would e.g. be `[1.0, 0.0,
    /// 0.0]`
    Color = 4, // nsi_sys::NSIType_t::NSITypeColor,
    /// Point, given as three 32-bit ([`f32`])floating point datas.
    Point = 5, // nsi_sys::NSIType_t::NSITypePoint,
    /// Vector, given as three 32-bit ([`f32`]) floating point datas.
    Vector = 6, // nsi_sys::NSIType_t::NSITypeVector,
    /// Normal vector, given as three 32-bit ([`f32`]) floating point
    /// datas.
    Normal = 7, // nsi_sys::NSIType_t::NSITypeNormal,
    /// Transformation matrix, given as 16 32-bit ([`f32`]) floating
    /// point datas.
    Matrix = 8, // nsi_sys::NSIType_t::NSITypeMatrix,
    /// Transformation matrix, given as 16 64-bit ([`f64`]) floating
    /// point datas.
    DoubleMatrix = 8 | 0x10, /* nsi_sys::NSIType_t::NSITypeMatrix |
                              * 0x10, */
    /// Raw (`*const T`) pointer.
    Pointer = 10, // nsi_sys::NSIType_t::NSITypePointer,
}

impl Type {
    #[inline]
    pub(crate) fn element_size(&self) -> usize {
        match self {
            Type::Invalid => 1, // avoid division by zero
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
