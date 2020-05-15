use crate::*;
use std::ffi::CString;

macro_rules! get_c_param_vec {
    ( $args_in:ident, $args_out: expr ) => {
        /* $args_out =
            $args_in.iter().map(|arg| to_nsi!(arg, args_in);).collect::<Vec<_>>();
        }*/
        // we create an array that holds our *const string pointers
        // so we can take *const *const strings of these before sending
        // to the FFI
        let mut string_vec = Vec::<*const _>::new();
        for arg_in in $args_in {
            let tmp = to_nsi!(arg_in, string_vec);
            $args_out.push(tmp);
        }
    };
}

macro_rules! to_nsi {
    ($self:ident, $string_vec:ident) => {{
        assert!(Type::Invalid != $self.type_nsi());

        let data_ptr = match $self.type_of {
            Type::String => {
                $string_vec.push($self.data.as_ptr_nsi());
                unsafe {
                    std::mem::transmute::<*const *const _, *const _>(
                        &(*$string_vec.last().unwrap()),
                    )
                }
            }
            _ => $self.data.as_ptr_nsi(),
        };

        nsi_sys::NSIParam_t {
            name: $self.name.as_ptr(),
            data: data_ptr,
            type_: $self.type_of as i32,
            arraylength: $self.array_length as i32,
            count: ($self.data.len_nsi()
                / $self.data.type_nsi().element_size()
                / $self.array_length) as u64,
            flags: $self.flags as std::os::raw::c_int,
        }
    }};
}

/// A type to specify optional arguments passed to ɴsɪ API calls.
///
/// This is used to pass variable argument lists. Most [`Context`]
/// methods accept a [`Vec`] of (optional) [`Arg`]s in the `args`
/// argument.
///
/// Also see the [ɴsɪ docmentation on optional
/// arguments](https://nsi.readthedocs.io/en/latest/c-api.html#passing-optional-arguments)
///
/// # String Caveats
///
/// The string value variant needs work. This currently only supports
/// [`CString`]. If you use another string type, e.g. [`&str`] or
/// [`String`] you'll get undefined behavior.
pub struct Arg<'a>
where
    dyn ToNSI: ConstraintNSI,
{
    pub(crate) name: CString,
    pub(crate) data: &'a dyn ToNSI,
    pub(crate) type_of: Type,
    // length of each element if an array type
    pub(crate) array_length: usize,
    pub(crate) flags: u32,
}

/*
impl Copy for Arg {}

impl Clone for Arg {
    fn clone(&self) -> Arg {
        Arg {
            nam: self.name,

        }
    }
}*/

/// A vector of (optional) [`Context`] method arguments.
pub type ArgVec<'a> = Vec<Arg<'a>>;

impl<'a> Arg<'a>
where
    dyn ToNSI: ConstraintNSI,
{
    #[inline]
    pub fn new(name: &str, data: &'a dyn ToNSI) -> Self
    where
        dyn ToNSI: ConstraintNSI,
    {
        Arg {
            name: CString::new(name).unwrap(),
            data,
            type_of: data.type_nsi(),
            array_length: data.tuple_len_nsi(),
            flags: 0,
        }
    }

    #[inline]
    // Sets the type of data this argument carries.
    // This method is called kind as type is a reserved
    // keyword in Rust
    pub fn kind(&'a mut self, type_of: Type) -> &'a Self {
        // FIXME: check if we fit in data.count() without remainder
        self.type_of = type_of;

        // Check that we fit.
        //assert!(self.data.len_nsi() / type_of.element_size() == self.array_length * self.count);

        self
    }

    #[inline]
    pub fn array_length(&'a mut self, array_length: usize) -> &'a Self {
        // Check that we fit.
        //assert!(self.data.len_nsi() / self.type_of.element_size() == array_length * self.count);

        self.array_length = array_length;
        self.flags |= nsi_sys::NSIParamIsArray;

        self
    }

    #[inline]
    pub fn per_face(&'a mut self) -> &'a mut Self {
        self.flags |= nsi_sys::NSIParamPerFace;
        self
    }

    #[inline]
    pub fn per_vertex(&'a mut self) -> &'a mut Self {
        self.flags |= nsi_sys::NSIParamPerVertex;
        self
    }

    #[inline]
    pub fn linear_interpolation(&'a mut self) -> &'a mut Self {
        self.flags |= nsi_sys::NSIParamInterpolateLinear;
        self
    }
}

/// Find the last occurrence of `name` in args
/// and extract that.
#[allow(unused_must_use)]
#[inline]
pub(crate) fn _extract_arg<'a>(args: &'a mut ArgVec, name: &str) -> Option<Arg<'a>> {
    let mut index: isize = -1;

    args.iter().enumerate().filter(|(i, arg)| {
        let found = arg.name == CString::new(name).unwrap();
        if found {
            index = *i as isize;
        }
        !found
    });

    match index {
        -1 => None,
        _ => Some(args.remove(index as usize)),
    }
}

/// Identifies an [`Arg`]’s data type.
#[derive(Copy, Clone, Debug)]
pub enum Type {
    /// Undefined type.
    Invalid = 0, // nsi_sys::NSIType_t::NSITypeInvalid,
    /// Single 32-bit ([`f32`]) floating point value.
    Float = 1, // nsi_sys::NSIType_t::NSITypeFloat,
    /// Single 64-bit ([`f64`]) floating point value.
    Double = 1 | 0x10, // nsi_sys::NSIType_t::NSITypeFloat | 0x10,
    /// Single 32-bit ([`i32`]) integer value.
    Integer = 2, // nsi_sys::NSIType_t::NSITypeInteger,
    /// A [`String`].
    String = 3, // nsi_sys::NSIType_t::NSITypeString,
    /// Color, given as three 32-bit ([`i32`]) floating point values,
    /// usually in the range `0..1`. Red would e.g. be `[1.0, 0.0,
    /// 0.0]`
    Color = 4, // nsi_sys::NSIType_t::NSITypeColor,
    /// Point, given as three 32-bit ([`f32`])floating point values.
    Point = 5, // nsi_sys::NSIType_t::NSITypePoint,
    /// Vector, given as three 32-bit ([`f32`]) floating point values.
    Vector = 6, // nsi_sys::NSIType_t::NSITypeVector,
    /// Normal vector, given as three 32-bit ([`f32`]) floating point
    /// values.
    Normal = 7, // nsi_sys::NSIType_t::NSITypeNormal,
    /// Transformation matrix, given as 16 32-bit ([`f32`]) floating
    /// point values.
    Matrix = 8, // nsi_sys::NSIType_t::NSITypeMatrix,
    /// Transformation matrix, given as 16 64-bit ([`f64`]) floating
    /// point values.
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

pub trait ToNSI {
    // this could solve our string issues!!!
    //fn new_nsi(&self) -> Self
    fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void;
    fn len_nsi(&self) -> usize;
    fn tuple_len_nsi(&self) -> usize;
    fn type_nsi(&self) -> Type;
}

pub trait ConstraintNSI {
    // No default impl for type_nsi() – this way we prevent the trait
    // being valid for any type by default
    fn no_op(&self);
}

/*
impl From<String> for dyn ToNSI {
    fn from(string: String) -> Self {
        CString::new(string).unwrap()
    }
}

impl From<&str> for dyn ToNSI {
    fn from(string: &str) -> Self {
        CString::new(string).unwrap()
    }
}*/

#[macro_export]
macro_rules! to_nsi_def {
    ($type:ty, $nsi_type:expr) => {
        impl ToNSI for $type
        where
            $type: ConstraintNSI,
        {
            #[inline]
            default fn type_nsi(&self) -> Type {
                $nsi_type
            }
        }

        impl ConstraintNSI for $type {
            #[inline]
            fn no_op(&self) {}
        }
    };
}

macro_rules! to_nsi_array_def {
    ($type:ty, $nsi_type:expr) => {
        impl<const N: usize> ToNSI for $type
        where
            $type: ConstraintNSI,
        {
            #[inline]
            default fn type_nsi(&self) -> Type {
                $nsi_type
            }
        }

        impl<const N: usize> ConstraintNSI for $type {
            #[inline]
            fn no_op(&self) {}
        }
    };
}

impl<T: ConstraintNSI> ToNSI for T {
    #[inline]
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self as *const _ as _
    }
    #[inline]
    default fn len_nsi(&self) -> usize {
        1
    }
    #[inline]
    default fn tuple_len_nsi(&self) -> usize {
        1
    }
    #[inline]
    default fn type_nsi(&self) -> Type {
        Type::Invalid
    }
}

to_nsi_def!(f32, Type::Float);
to_nsi_def!(f64, Type::Double);
to_nsi_def!(i32, Type::Integer);
to_nsi_def!(u64, Type::Integer);

impl<T> ToNSI for *const T
where
    *const T: ConstraintNSI,
{
    #[inline]
    default fn type_nsi(&self) -> Type {
        Type::Pointer
    }
}

impl<T> ConstraintNSI for *const T {
    #[inline]
    fn no_op(&self) {}
}

impl ToNSI for CString
where
    CString: ConstraintNSI,
{
    #[inline]
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    #[inline]
    default fn type_nsi(&self) -> Type {
        Type::String
    }
}

impl ConstraintNSI for CString {
    #[inline]
    fn no_op(&self) {}
}

impl<T: ConstraintNSI, const N: usize> ToNSI for [T; N]
where
    [T; N]: ConstraintNSI,
{
    #[inline]
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    #[inline]
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    #[inline]
    default fn tuple_len_nsi(&self) -> usize {
        1
    }
    #[inline]
    default fn type_nsi(&self) -> Type {
        Type::Invalid
    }
}

to_nsi_array_def!([f32; N], Type::Float);
to_nsi_array_def!([f64; N], Type::Double);
to_nsi_array_def!([i32; N], Type::Integer);
to_nsi_array_def!([i64; N], Type::Integer);

// FIXME Sting arrays

impl ToNSI for [f32; 3] {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::Color
    }
}

impl ToNSI for [f32; 16] {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::Matrix
    }
}

impl ToNSI for [f64; 16] {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::DoubleMatrix
    }
}

/*
impl ToNSI for dyn FnMut(Context, i32) + 'static {
    #[inline]
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self as *const _ as _
    }
    #[inline]
    default fn type_nsi(&self) -> Type {
        Type::Pointer
    }
}*/

impl<T: ConstraintNSI> ToNSI for Vec<T>
where
    Vec<T>: ConstraintNSI,
{
    #[inline]
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    #[inline]
    default fn len_nsi(&self) -> usize {
        self.len()
    }

    #[inline]
    default fn tuple_len_nsi(&self) -> usize {
        1
    }
    #[inline]
    default fn type_nsi(&self) -> Type {
        Type::Invalid
    }
}

to_nsi_def!(Vec<f32>, Type::Float);
to_nsi_def!(Vec<f64>, Type::Double);
to_nsi_def!(Vec<i32>, Type::Integer);
to_nsi_def!(Vec<u32>, Type::Integer);

impl ToNSI for Vec<[f32; 3]> {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::Color
    }
}

impl ToNSI for Vec<[f32; 16]> {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::Matrix
    }
}

impl ToNSI for Vec<[f64; 16]> {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::DoubleMatrix
    }
}

impl<T> ToNSI for Vec<*const T> {
    #[inline]
    fn type_nsi(&self) -> Type {
        Type::Pointer
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

/// A macro to create a [`CStr`] (&[`CString`]) from a [`Vec`]<[`u8`]>.
///
/// ```
/// // Create rendering context.
/// let ctx = nsi::Context::new(&vec![nsi::arg!(
///     "streamfilename",
///     nsi::c_str!("stdout")
/// )])
/// .expect("Could not create NSI context.");
/// ```
#[macro_export]
macro_rules! c_str {
    ($str: expr) => {
        &std::ffi::CString::new($str).unwrap()
    };
}

/// A macro to create an Argument aka: [`Arg::new()`].
///
/// ```
/// // Create rendering context.
/// let ctx = nsi::Context::new(&vec![nsi::arg!(
///     "streamfilename",
///     nsi::c_str!("stdout")
/// )])
/// .expect("Could not create NSI context.");
/// ```
#[macro_export]
macro_rules! arg {
    ($token:expr, $value:expr) => {
        nsi::Arg::new($token, $value)
    };
}
