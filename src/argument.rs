/// An argument type to specify optional parameters and/or attributes
/// passed to ɴsɪ API calls.
///
/// This is used to pass variable parameter lists. Most [`Context`]
/// methods accept a [`Vec`] of (optional) [`Arg`]s in the `args`
/// parameter.
///
/// Also see the [ɴsɪ docmentation on optional
/// parameters](https://nsi.readthedocs.io/en/latest/c-api.html#passing-optional-parameters)
pub struct Arg<'a> {
    name: CString,
    data: &'a dyn ToNSI,
    type_of: Type,
    array_length: usize, // length of each element
    count: usize,        // number of elements
    flags: u32,
}

pub type ArgVec<'a> = Vec<Arg<'a>>;

fn get_c_param_vec<'a>(
    args_in: &'a ArgVec,
    args_out: &'a mut Vec<nsi_sys::NSIParam_t>,
) {
    *args_out =
        args_in.iter().map(|arg| arg.to_nsi()).collect::<Vec<_>>();
}

impl<'a> Arg<'a> {
    pub fn new(name: &str, data: &'a dyn ToNSI) -> Self {
        Arg {
            name: CString::new(name).unwrap(),
            data,
            type_of: data.type_nsi(),
            array_length: 1,
            count: data.len_nsi() / data.type_nsi().element_size(),
            flags: 0,
        }
    }

    fn to_nsi(&'a self) -> nsi_sys::NSIParam_t {
        // Check that we fit without remainder.
        assert!(if nsi_sys::NSIParamIsArray & self.flags == 0 {
            self.data.len_nsi() % self.count == 0
        } else {
            // Array case.
            self.data.len_nsi() % (self.array_length * self.count) == 0
        });

        nsi_sys::NSIParam_t {
            name: self.name.as_ptr(),
            data: self.data.as_ptr_nsi(),
            type_: self.type_of as i32,
            arraylength: self.array_length as i32,
            count: self.count,
            flags: self.flags as std::os::raw::c_int,
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

    pub fn set_array_length(mut self, array_length: usize) -> Self {
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
    pub fn element_size(&self) -> usize {
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
    fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void;
    fn len_nsi(&self) -> usize;
    fn type_nsi(&self) -> Type;
}

impl<T> ToNSI for T {
    default fn as_ptr_nsi(&self) -> *const ::std::os::raw::c_void {
        self as *const _ as _
    }
    default fn len_nsi(&self) -> usize {
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

impl ToNSI for String {
    default fn type_nsi(&self) -> Type {
        Type::String
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

impl ToNSI for Vec<String> {
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

#[macro_export]
macro_rules! no_arg {
    () => {
        &nsi::ArgVec::new()
    };
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
