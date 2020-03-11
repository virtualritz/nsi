use std::ffi::CString;

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
            count: data.len_nsi(),
            flags: 0,
        }
    }

    fn to_nsi(&'a self) -> nsi_sys::NSIParam_t {
        assert!(
            self.data.len_nsi() % (self.array_length * self.count) == 0
        );
        nsi_sys::NSIParam_t {
            name: self.name.as_ptr(),
            data: self.data.as_ptr_nsi(),
            type_: self.type_of as i32,
            arraylength: self.array_length as i32,
            count: self.count,
            flags: self.flags as std::os::raw::c_int,
        }
    }

    pub fn type_of(mut self, type_of: Type) -> Self {
        // FIXME: check if we fit in data.count() without remainder
        self.type_of = type_of;
        self
    }

    pub fn count(mut self, count: usize) -> Self {
        assert!(count * self.array_length <= self.data.len_nsi());
        assert!(self.data.len_nsi() % count == 0);

        self.count = count;
        self.array_length = self.data.len_nsi() / count;
        self
    }

    pub fn array_length(mut self, array_length: usize) -> Self {
        assert!(self.count * array_length <= self.data.len_nsi());
        assert!(self.data.len_nsi() % array_length == 0);

        self.array_length = array_length;
        self.count = self.data.len_nsi() / array_length;
        self
    }

    pub fn flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Type {
    None = -1,         // nsi_sys::NSIType_t::NSITypeInvalid,
    Float = 0,         // nsi_sys::NSIType_t::NSITypeFloat,
    Double = 1 | 0x10, // nsi_sys::NSIType_t::NSITypeFloat | 0x10,
    Integer = 2,       // nsi_sys::NSIType_t::NSITypeInteger,
    String = 3,        // nsi_sys::NSIType_t::NSITypeString,
    Color = 4,         // nsi_sys::NSIType_t::NSITypeColor,
    Point = 5,         // nsi_sys::NSIType_t::NSITypePoint,
    Vector = 6,        // nsi_sys::NSIType_t::NSITypeVector,
    Normal = 7,        // nsi_sys::NSIType_t::NSITypeNormal,
    Matrix = 8,        // nsi_sys::NSIType_t::NSITypeMatrix,
    DoubleMatrix = 8 | 0x10, /* nsi_sys::NSIType_t::NSITypeMatrix |
                        * 0x10, */
    Pointer = 10, // nsi_sys::NSIType_t::NSITypePointer,
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
        Type::None
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

impl ToNSI for [f32; 3] {
    default fn type_nsi(&self) -> Type {
        Type::Color
    }
}

impl ToNSI for [f32; 16] {
    default fn type_nsi(&self) -> Type {
        Type::Matrix
    }
}

impl ToNSI for [f64; 16] {
    default fn type_nsi(&self) -> Type {
        Type::DoubleMatrix
    }
}

impl<T> ToNSI for *const T {
    default fn type_nsi(&self) -> Type {
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
        Type::None
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

#[test]
fn test() {
    let single = Arg::new("foo", &10.0f32);
    let vector = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
    let array = Arg::new("bar", &vector).count(2); // 2x2 array of f32

    let mut result_vec = Vec::<nsi_sys::NSIParam_t>::new();
    get_c_param_vec(&vec![single, array], &mut result_vec);

    dbg!(result_vec);
}
