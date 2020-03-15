use std::ffi::CString;
use crate::*;


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
    }
}

macro_rules! to_nsi {
    ($self:ident, $string_vec:ident) => {{
        // Check that we fit without remainder.
        assert!(if nsi_sys::NSIParamIsArray & $self.flags == 0 {
            $self.data.len_nsi() % $self.count == 0
        } else {
            // Array case.
            $self.data.len_nsi() % ($self.array_length * $self.count) == 0
        });

        let data_ptr = match $self.type_of {
            Type::String => {
                $string_vec.push($self.data.as_ptr_nsi());
                unsafe { std::mem::transmute::<*const *const _, *const _>(&(*$string_vec.first().unwrap())) }
            },
            _ => $self.data.as_ptr_nsi(),
        };

        nsi_sys::NSIParam_t {
            name: $self.name.as_ptr(),
            data: data_ptr,
            type_: $self.type_of as i32,
            arraylength: $self.array_length as i32,
            count: $self.count,
            flags: $self.flags as std::os::raw::c_int,
        }
    }}
}

/// An argument type to specify optional parameters and/or attributes
/// passed to ɴsɪ API calls.
///
/// This is used to pass variable parameter lists. Most [`Context`]
/// methods accept a [`Vec`] of (optional) [`Arg`]s in the `args`
/// parameter.
///
/// Also see the [ɴsɪ docmentation on optional
/// parameters](https://nsi.readthedocs.io/en/latest/c-api.html#passing-optional-parameters)
///
/// # String Caveats
///
/// The string value variant needs work. This currently only supports
/// [`CString`]. If you use another string type, e.g. [`&str`] or
/// [`String`] you'll get undefined behavior.
pub struct Arg<'a> {
    pub (crate) name: CString,
    pub (crate) data: &'a dyn ToNSI,
    pub (crate) type_of: Type,
    pub (crate) array_length: usize, // length of each element if an array type
    pub (crate) count: usize,        // number of elements
    pub (crate) flags: u32,
}

/// A vector of (optional) [`Context`] method parameters.
pub type ArgVec<'a> = Vec<Arg<'a>>;


impl<'a> Arg<'a> {
    pub fn new(name: &str, data: &'a dyn ToNSI) -> Self {
        Arg {
            name: CString::new(name).unwrap(),
            data,
            type_of: data.type_nsi(),
            array_length: data.tuple_len_nsi(),
            count: data.len_nsi() / data.type_nsi().element_size(),
            flags: 0,
        }
    }



    pub fn set_type(mut self, type_of: Type) -> Self {
        // FIXME: check if we fit in data.count() without remainder
        self.type_of = type_of;

        // Type can change count -> re-calculate.

        if nsi_sys::NSIParamIsArray & self.flags == 0 {
            self.count =
                self.data.len_nsi() / self.type_of.element_size();

            // Check that we fit w/o remainder.
            assert!(
                self.data.len_nsi() % self.type_of.element_size() == 0
            );
        } else {
            // This is an array.
            self.count = self.data.len_nsi()
                / self.type_of.element_size()
                / self.array_length;

            // Check that we fit w/o remainder.
            assert!(
                self.data.len_nsi()
                    % self.type_of.element_size()
                    % self.array_length
                    == 0
            );
        }

        self
    }

    fn set_array_length(mut self, array_length: usize) -> Self {
        // Make sure we fit at all.
        assert!(
            self.data.len_nsi()
                / self.type_of.element_size()
                / array_length
                >= 1
        );

        self.array_length = array_length;
        self.flags |= nsi_sys::NSIParamIsArray;

        // Array length can change count -> re-calculate
        self.count = self.data.len_nsi()
            / self.type_of.element_size()
            / self.array_length;

        // Check that we fit w/o remainder.
        assert!(
            self.data.len_nsi()
                % self.type_of.element_size()
                % self.array_length
                == 0
        );

        self
    }

    pub fn set_tuple_len(self, tuple_length: usize) -> Self {
        self.set_array_length(tuple_length)
    }

    pub fn set_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }
}

/// Identifies an [`Arg`]’s data type.
#[derive(Copy, Clone, Debug)]
pub enum Type {
    /// Undefined type.
    Invalid = -1,      // nsi_sys::NSIType_t::NSITypeInvalid,
    /// Single 32-bit ([`f32`]) floating point value.
    Float = 0,         // nsi_sys::NSIType_t::NSITypeFloat,
    /// Single 64-bit ([`f64`]) floating point value.
    Double = 1 | 0x10, // nsi_sys::NSIType_t::NSITypeFloat | 0x10,
    /// Single 32-bit ([`i32`]) integer value.
    Integer = 2,       // nsi_sys::NSIType_t::NSITypeInteger,
    /// A [`String`].
    String = 3,        // nsi_sys::NSIType_t::NSITypeString,
    /// Color, given as three 32-bit ([`i32`]) floating point values, usually in the range `0..1`. Red would e.g. be `[1.0, 0.0, 0.0]`
    Color = 4,         // nsi_sys::NSIType_t::NSITypeColor,
    /// Point, given as three 32-bit ([`f32`])floating point values.
    Point = 5,         // nsi_sys::NSIType_t::NSITypePoint,
    /// Vector, given as three 32-bit ([`f32`]) floating point values.
    Vector = 6,        // nsi_sys::NSIType_t::NSITypeVector,
    /// Normal vector, given as three 32-bit ([`f32`]) floating point values.
    Normal = 7,        // nsi_sys::NSIType_t::NSITypeNormal,
    /// Transformation matrix, given as 16 32-bit ([`f32`]) floating point values.
    Matrix = 8,        // nsi_sys::NSIType_t::NSITypeMatrix,
    /// Transformation matrix, given as 16 64-bit ([`f64`]) floating point values.
    DoubleMatrix = 8 | 0x10, /* nsi_sys::NSIType_t::NSITypeMatrix |
                        * 0x10, */
    /// Raw (`*const T`) pointer.
    Pointer = 10, // nsi_sys::NSIType_t::NSITypePointer,
}

impl Type {
    fn element_size(&self) -> usize {
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

impl<T> ToNSI for T {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self as *const _ as _
    }
    default fn len_nsi(&self) -> usize {
        1
    }
    default fn tuple_len_nsi(&self) -> usize {
        1
    }
    default fn type_nsi(&self) -> Type {
        Type::Invalid
    }
}

impl ToNSI for f32 {
    default fn type_nsi(&self) -> Type {
        Type::Float
    }
}

impl ToNSI for f64 {
    default fn type_nsi(&self) -> Type {
        Type::Double
    }
}

impl ToNSI for i32 {
    default fn type_nsi(&self) -> Type {
        Type::Integer
    }
}

impl ToNSI for CString {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }

    default fn type_nsi(&self) -> Type {
        Type::String
    }
}

#[cfg(features="algebra-nalgebra")]
impl ToNSI for nalgebra::Matrix4<f32> {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        (self.data).as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::Matrix
    }
}

#[cfg(feature="algebra-nalgebra")]
impl ToNSI for nalgebra::Matrix4<f64> {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.data.as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::DoubleMatrix
    }
}

impl<T, const N: usize> ToNSI for [T; N] {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::Invalid
    }
}

impl<const N: usize> ToNSI for [f32; N] {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::Float
    }
}

impl<const N: usize> ToNSI for [i32; N] {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::Integer
    }
}

impl<const N: usize> ToNSI for [f64; N] {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::Double
    }
}

impl<const N: usize> ToNSI for [String; N] {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    default fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::String
    }
}

impl ToNSI for [f32; 3] {
    fn type_nsi(&self) -> Type {
        Type::Color
    }
}

impl ToNSI for [f32; 16] {
    fn type_nsi(&self) -> Type {
        Type::Matrix
    }
}

impl ToNSI for [f64; 16] {
    fn type_nsi(&self) -> Type {
        Type::DoubleMatrix
    }
}

impl<T> ToNSI for *const T {
    fn type_nsi(&self) -> Type {
        Type::Pointer
    }
}

impl<T> ToNSI for Vec<T> {
    fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self.as_ptr() as _
    }
    fn len_nsi(&self) -> usize {
        self.len()
    }
    default fn type_nsi(&self) -> Type {
        Type::Invalid
    }
}

impl ToNSI for Vec<f32> {
    default fn type_nsi(&self) -> Type {
        Type::Float
    }
}

impl ToNSI for Vec<f64> {
    default fn type_nsi(&self) -> Type {
        Type::Double
    }
}

impl ToNSI for Vec<i32> {
    default fn type_nsi(&self) -> Type {
        Type::Integer
    }
}

impl ToNSI for Vec<CString> {
    default fn type_nsi(&self) -> Type {
        Type::String
    }
}

impl ToNSI for Vec<[f32; 3]> {
    default fn type_nsi(&self) -> Type {
        Type::Color
    }
}

impl ToNSI for Vec<[f32; 16]> {
    default fn type_nsi(&self) -> Type {
        Type::Matrix
    }
}

impl ToNSI for Vec<[f64; 16]> {
    default fn type_nsi(&self) -> Type {
        Type::DoubleMatrix
    }
}

impl<T> ToNSI for Vec<*const T> {
    default fn type_nsi(&self) -> Type {
        Type::Pointer
    }
}



/// A macro to specify an empty [`ArgVec`] to a [`Context`] method
/// that supports optional parameters.
///
/// ```
/// // Create rendering context.
/// let context = nsi::Context::new(nsi::no_arg!()).unwrap();
/// ```
#[macro_export]
macro_rules! no_arg {
    () => {
        &nsi::ArgVec::new()
    }
}

/// A macro to create a [`CStr`] (&[`CString`]) from a [`Vec`]<[`u8`]>.
///
/// ```
/// // Create rendering context.
/// let context = nsi::Context::new(nsi::no_arg!()).unwrap();
/// ```
#[macro_export]
macro_rules! c_str {
    ($str: expr) => {
        &std::ffi::CString::new($str).unwrap()
    }
}

#[macro_export]
macro_rules! arg {
    ($token:expr, $value:expr) => {
        nsi::Arg::new($token, $value)
    }
}

/*
#[test]
fn test() {
    let single = Arg::new("foo", &10.0f32);
    let vector = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
    let array = Arg::new("bar", &vector).count(2); // 2x2 array of f32

    let mut result_vec = Vec::<nsi_sys::NSIParam_t>::new();
    get_c_param_vec(&vec![single, array], &mut result_vec);

    dbg!(result_vec);
}*/