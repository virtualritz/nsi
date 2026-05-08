//! Optional arguments passed to methods of an ɴsɪ context.
use enum_dispatch::enum_dispatch;
use nsi_sys::*;
use std::{
    ffi::{CString, c_void},
    marker::PhantomData,
    pin::Pin,
};
use ustr::{Ustr, ustr};

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
/// [`Context`].
pub type ArgSlice<'a, 'b> = [Arg<'a, 'b>];

/// A vector of (optional) arguments passed to a method of
/// [`Context`].
pub type ArgVec<'a, 'b> = Vec<Arg<'a, 'b>>;

/// An (optional) argument passed to a method of
/// [`Context`].
#[derive(Debug, Clone)]
pub struct Arg<'a, 'b> {
    pub(crate) name: Ustr,
    pub(crate) data: ArgData<'a, 'b>,
    // Length of each element if an array type.
    pub(crate) array_length: usize,
    // Number of elements.
    pub(crate) flags: i32,
}

impl<'a, 'b> Arg<'a, 'b> {
    #[inline]
    pub fn new(name: &str, data: ArgData<'a, 'b>) -> Self {
        Arg {
            name: ustr(name),
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
#[derive(Debug, Clone)]
pub enum ArgData<'a, 'b> {
    /// Single [`f32`] value.
    F32,
    /// An `[`[`f32`]`]` slice.
    F32Slice(F32Slice<'a>),
    /// Single [`f64`] value.
    F64,
    /// An `[`[`f64`]`]` slice.
    F64Slice(F64Slice<'a>),
    /// Single [`i32`] value.
    I32,
    /// An `[`[`i32`]`]` slice.
    I32Slice(I32Slice<'a>),
    /// Single [`i64`] value.
    I64,
    /// An `[`[`i64`]`]` slice.
    I64Slice(I64Slice<'a>),
    /// A [`String`].
    String(String),
    /// A `[`[`String`]`]` slice.
    StringSlice(StringSlice),
    /// Color in linear space, given as a red, green, blue triplet
    /// of [`f32`] values; usually in the range `0..1`.
    Color(Color<'a>),
    /// A flat `[`[`f32`]`]` slice of colors (`len % 3 == 0`).
    ColorSlice(ColorSlice<'a>),
    /// Point, given as three [`f32`] values.
    Point(Point<'a>),
    /// A flat `[`[`f32`]`]` slice of points (`len % 3 == 0`).
    PointSlice(PointSlice<'a>),
    /// Vector, given as three [`f32`] values.
    Vector(Vector<'a>),
    /// A flat `[`[`f32`]`]` slice of vectors (`len % 3 == 0`).
    VectorSlice(VectorSlice<'a>),
    /// Normal vector, given as three [`f32`] values.
    Normal(Normal<'a>),
    /// A flat `[`[`f32`]`]` slice of normals (`len % 3 == 0`).
    NormalSlice(NormalSlice<'a>),
    /// Row-major, 4×4 transformation matrix, given as 16 [`f32`] values.
    MatrixF32(MatrixF32<'a>),
    /// A flat `[`[`f32`]`]` slice of matrices (`len % 16 == 0`).
    MatrixF32Slice(MatrixF32Slice<'a>),
    /// Row-major, 4×4 transformation matrix, given as 16 [`f64`] values.
    MatrixF64(MatrixF64<'a>),
    /// A flat `[`[`f64`]`]` slice of matrices (`len % 16 == 0`).
    MatrixF64Slice(MatrixF64Slice<'a>),
    /// Reference *with* lifetime guarantees.
    ///
    /// This gets converted to a raw pointer when passed
    /// through the FFI boundary.
    ///
    /// ```
    /// # use nsi_ffi_wrap as nsi;
    /// let ctx = nsi::Context::new(None).unwrap();
    ///
    /// // Lots of scene setup omitted ...
    ///
    /// // Setup a custom output driver and send
    /// // a payload to it through the FFI boundary.
    /// ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    /// ctx.connect("driver", None, "beauty", "outputdrivers", None);
    ///
    /// struct Payload {
    ///     some_data: u32,
    /// }
    ///
    /// // Must use heap allocation for stable address
    /// let payload = Box::new(Payload { some_data: 42 });
    /// ctx.set_attribute(
    ///     "driver",
    ///     &[
    ///         nsi::string!("drivername", "custom_driver"),
    ///         // Payload gets sent as raw pointer through
    ///         // the FFI boundary. The Box ensures stable address.
    ///         nsi::reference!("payload", &payload),
    ///     ],
    /// );
    ///
    /// // We need to explicitly call drop here as
    /// // ctx's lifetime is pegged to that of payload.
    /// drop(ctx);
    /// ```
    Reference(Reference<'b>),
    /// A `[`[`Reference`]`]` slice.
    ReferenceSlice(ReferenceSlice<'b>),
    /// A callback.
    Callback(Callback<'b>),
}

macro_rules! nsi_data_def {
    ($type: ty, $name: ident, $nsi_type: expr) => {
        /// See [`ArgData`] for details.
        #[derive(Debug, Clone, PartialEq)]
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
        /// See [`ArgData`] for details.
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name<'a> {
            data: &'a [$type],
        }

        impl<'a> $name<'a> {
            pub fn new(data: &'a [$type]) -> Self {
                //debug_assert_eq!(0, data.len() % $nsi_type.elemensize());
                Self { data }
            }
        }

        impl<'a> ArgDataMethods for $name<'a> {
            fn type_(&self) -> Type {
                $nsi_type
            }

            fn len(&self) -> usize {
                self.data.len() // / $nsi_type.elemensize()
            }

            fn as_c_ptr(&self) -> *const c_void {
                self.data.as_ptr() as _
            }
        }
    };
}

macro_rules! nsi_tuple_data_array_def {
    ($type: ty, $name: ident, $nsi_type: expr, $len: expr ) => {
        /// See [`ArgData`] for details.
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name<'a> {
            data: &'a [[$type; $len]],
        }

        impl<'a> $name<'a> {
            pub fn new(data: &'a [[$type; $len]]) -> Self {
                //debug_assert_eq!(0, data.len() % $nsi_type.elemensize());
                Self { data }
            }
        }

        impl<'a> ArgDataMethods for $name<'a> {
            fn type_(&self) -> Type {
                $nsi_type
            }

            fn len(&self) -> usize {
                self.data.len() // / $nsi_type.elemensize()
            }

            fn as_c_ptr(&self) -> *const c_void {
                self.data.as_ptr() as _
            }
        }
    };
}

macro_rules! nsi_tuple_data_def {
    ($type: tt, $len: expr, $name: ident, $nsi_type: expr) => {
        /// See [`ArgData`] for details.
        #[derive(Debug, Clone, PartialEq)]
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

nsi_data_def!(f32, F32, Type::F32);
nsi_data_def!(f64, F64, Type::F64);
nsi_data_def!(i32, I32, Type::I32);
nsi_data_def!(i64, I64, Type::I64);

/// See [`ArgData`] for details.
/// A reference to data that will be passed through FFI.
///
/// # Safety
/// The referenced data must outlive the NSI context that uses this reference.
/// The data must remain at a stable memory address.
///
/// This type now properly enforces pinning by requiring heap-allocated data
/// through Box, Arc, or similar types that guarantee stable addresses.
#[derive(Debug, Clone)]
pub struct Reference<'a> {
    data: *const c_void,
    _marker: PhantomData<&'a ()>,
}

// SAFETY: Reference only contains a pointer and doesn't dereference it.
// The actual safety depends on the lifetime parameter being correct.
unsafe impl Send for Reference<'static> {}
unsafe impl Sync for Reference<'static> {}

/// Trait for types that can be safely converted to a Reference.
/// This is implemented only for types that guarantee stable memory addresses.
pub trait StableDeref<'a> {
    /// Get a stable pointer to the data
    fn stable_deref(&self) -> *const c_void;
}

impl<'a, T: ?Sized> StableDeref<'a> for &'a Box<T> {
    fn stable_deref(&self) -> *const c_void {
        self.as_ref() as *const T as *const c_void
    }
}

impl<'a, T: ?Sized> StableDeref<'a> for &'a Arc<T> {
    fn stable_deref(&self) -> *const c_void {
        self.as_ref() as *const T as *const c_void
    }
}

impl<'a, T: ?Sized> StableDeref<'a> for &'a Pin<Box<T>> {
    fn stable_deref(&self) -> *const c_void {
        self.as_ref().get_ref() as *const T as *const c_void
    }
}

use std::sync::Arc;

impl<'a> Reference<'a> {
    /// Create a reference from any type that implements StableDeref.
    /// This includes &Box<T>, &Arc<T>, and &Pin<Box<T>>.
    pub fn new<S: StableDeref<'a>>(data: S) -> Self {
        let ptr = data.stable_deref();
        debug_assert!(!ptr.is_null(), "Reference created with null pointer");

        Self {
            data: ptr,
            _marker: PhantomData,
        }
    }

    /// Create a reference from a Box.
    ///
    /// Box guarantees a stable heap address, making it safe for FFI.
    #[allow(clippy::borrowed_box)]
    pub fn from_box<T: ?Sized>(data: &'a Box<T>) -> Self {
        let ptr = data.as_ref() as *const T as *const c_void;
        debug_assert!(!ptr.is_null(), "Reference created with null pointer");

        Self {
            data: ptr,
            _marker: PhantomData,
        }
    }

    /// Create a reference from an Arc.
    ///
    /// Arc guarantees a stable heap address, making it safe for FFI.
    pub fn from_arc<T: ?Sized>(data: &'a Arc<T>) -> Self {
        let ptr = data.as_ref() as *const T as *const c_void;
        debug_assert!(!ptr.is_null(), "Reference created with null pointer");

        Self {
            data: ptr,
            _marker: PhantomData,
        }
    }

    /// Create a reference from a pinned Box.
    ///
    /// This is the safest option as it guarantees the data cannot be moved.
    pub fn from_pin_box<T: ?Sized>(data: &'a Pin<Box<T>>) -> Self {
        let ptr = data.as_ref().get_ref() as *const T as *const c_void;
        debug_assert!(!ptr.is_null(), "Reference created with null pointer");

        Self {
            data: ptr,
            _marker: PhantomData,
        }
    }

    /// Create a reference from data with a stable address.
    ///
    /// # Safety
    /// The caller must guarantee that `data` has a stable address for its entire
    /// lifetime. This is automatically true for:
    /// - Heap allocated types (Box, Arc, Vec's data, String's data)
    /// - Static data
    /// - Pinned data
    ///
    /// This is NOT safe for:
    /// - Stack allocated data that might move
    /// - Data inside collections that might reallocate
    pub unsafe fn from_stable<T: ?Sized>(data: &'a T) -> Self {
        let ptr = data as *const T as *const c_void;
        debug_assert!(!ptr.is_null(), "Reference created with null pointer");

        Self {
            data: ptr,
            _marker: PhantomData,
        }
    }

    /// Create a reference from a raw pointer.
    ///
    /// # Safety
    /// The caller must ensure that:
    /// 1. The pointer is valid and non-null
    /// 2. The pointed-to data outlives 'a
    /// 3. The data won't be moved or freed while this reference exists
    pub unsafe fn from_ptr(ptr: *const c_void) -> Self {
        debug_assert!(!ptr.is_null(), "Reference created with null pointer");
        Self {
            data: ptr,
            _marker: PhantomData,
        }
    }
}

impl<'a> ArgDataMethods for Reference<'a> {
    fn type_(&self) -> Type {
        Type::Reference
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

unsafe impl Send for Callback<'static> {}
unsafe impl Sync for Callback<'static> {}

/// See [`ArgData`] for details.
#[derive(Debug, Clone)]
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
        Type::Reference
    }

    fn len(&self) -> usize {
        1
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data
    }
}

/// See [`ArgData`] for details.
#[derive(Debug, Clone)]
pub struct String {
    #[allow(dead_code)]
    data: CString,
    // The FFI API needs a pointer to a C string.
    pointer: *const c_void,
}

unsafe impl Send for String {}
unsafe impl Sync for String {}

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

    fn as_c_ptr(&self) -> *const c_void {
        &self.pointer as *const *const c_void as _
    }
}

nsi_data_array_def!(f32, F32Slice, Type::F32);
nsi_data_array_def!(f64, F64Slice, Type::F64);
nsi_data_array_def!(i32, I32Slice, Type::I32);
nsi_data_array_def!(i64, I64Slice, Type::I64);
nsi_tuple_data_array_def!(f32, ColorSlice, Type::Color, 3);
nsi_tuple_data_array_def!(f32, PointSlice, Type::Point, 3);
nsi_tuple_data_array_def!(f32, VectorSlice, Type::Vector, 3);
nsi_tuple_data_array_def!(f32, NormalSlice, Type::Normal, 3);
nsi_tuple_data_array_def!(f32, MatrixF32Slice, Type::MatrixF32, 16);
nsi_tuple_data_array_def!(f64, MatrixF64Slice, Type::MatrixF64, 16);

/// See [`ArgData`] for details.
#[derive(Debug, Clone)]
pub struct ReferenceSlice<'a> {
    data: Vec<*const c_void>,
    _marker: PhantomData<&'a ()>,
}

unsafe impl Send for ReferenceSlice<'static> {}
unsafe impl Sync for ReferenceSlice<'static> {}

impl<'a> ReferenceSlice<'a> {
    pub fn new<T>(data: &'a [&'a T]) -> Self {
        debug_assert_eq!(0, data.len() % Type::Reference.elemensize());

        Self {
            data: data.iter().map(|r| r as *const _ as _).collect(),
            _marker: PhantomData,
        }
    }
}

impl<'a> ArgDataMethods for ReferenceSlice<'a> {
    fn type_(&self) -> Type {
        Type::Reference
    }

    fn len(&self) -> usize {
        self.data.len() / Type::Reference.elemensize()
    }

    fn as_c_ptr(&self) -> *const c_void {
        self.data.as_ptr() as _
    }
}

/// See [`ArgData`] for details.
#[derive(Debug, Clone)]
pub struct StringSlice {
    #[allow(dead_code)]
    data: Vec<CString>,
    pointer: Vec<*const c_void>,
}

unsafe impl Send for StringSlice {}
unsafe impl Sync for StringSlice {}

impl StringSlice {
    pub fn new<T: Into<Vec<u8>> + Copy>(data: &[T]) -> Self {
        let data = data
            .iter()
            .map(|s| CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let pointer = data.iter().map(|s| s.as_ptr() as _).collect();

        StringSlice { data, pointer }
    }
}

impl ArgDataMethods for StringSlice {
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
nsi_tuple_data_def!(f32, 16, MatrixF32, Type::MatrixF32);
nsi_tuple_data_def!(f64, 16, MatrixF64, Type::MatrixF64);

/// Identifies an [`Arg`]’s data type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub(crate) enum Type {
    /// A single [`f32`] value.
    F32 = NSIType::F32 as _,
    /// A single [`f64`] value.
    F64 = NSIType::F64 as _,
    /// Single [`i32`] value.
    I32 = NSIType::I32 as _,
    /// Single [`i64`] value.
    I64 = NSIType::I64 as _,
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
    MatrixF32 = NSIType::MatrixF32 as _,
    /// Transformation matrix, given as 16 [`f64`] values.
    MatrixF64 = NSIType::MatrixF64 as _,
    /// Raw (`*const T`) pointer.
    Reference = NSIType::Pointer as _,
}

impl Type {
    /// Returns the number of components of the resp. type.
    #[inline]
    pub(crate) fn elemensize(&self) -> usize {
        match self {
            Type::F32 => 1,
            Type::F64 => 1,
            Type::I32 => 1,
            Type::I64 => 1,
            Type::String => 1,
            Type::Color => 3,
            Type::Point => 3,
            Type::Vector => 3,
            Type::Normal => 3,
            Type::MatrixF32 => 16,
            Type::MatrixF64 => 16,
            Type::Reference => 1,
        }
    }
}

/// Create a [`F32`] argument.
#[macro_export]
macro_rules! f32 {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::F32::new($value)))
    };
}

/// Create a [`F32Slice`] array argument.
#[macro_export]
macro_rules! f32_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::F32Slice::new($value)))
    };
}

/// Create a [`F64`] precision argument.
#[macro_export]
macro_rules! f64 {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::F64::new($value)))
    };
}

/// Create a [`F64Slice`] precision array argument.
#[macro_export]
macro_rules! f64_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::F64Slice::new($value)))
    };
}

/// Create a [`I32`] argument.
#[macro_export]
macro_rules! i32 {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::I32::new($value)))
    };
}

/// Create a [`I32Slice`] array argument.
#[macro_export]
macro_rules! i32_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::I32Slice::new($value)))
    };
}

/// Create a [`I64`] argument.
#[macro_export]
macro_rules! i64 {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::I64::new($value)))
    };
}

/// Create a [`I64Slice`] array argument.
#[macro_export]
macro_rules! i64_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::I64Slice::new($value)))
    };
}

/// Create a [`Color`] argument.
#[macro_export]
macro_rules! color {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Color::new($value)))
    };
}

/// Create a [`ColorSlice`] array argument.
#[macro_export]
macro_rules! color_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::ColorSlice::new($value)))
    };
}

/// Create a [`Point`] argument.
#[macro_export]
macro_rules! point {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Point::new($value)))
    };
}

/// Create a [`PointSlice`] array argument.
#[macro_export]
macro_rules! point_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::PointSlice::new($value)))
    };
}

/// Create a [`Vector`] argument.
#[macro_export]
macro_rules! vector {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Vector::new($value)))
    };
}

/// Create a [`VectorSlice`] array argument.
#[macro_export]
macro_rules! vector_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::VectorSlice::new($value)))
    };
}

/// Create a [`Normal`] argument.
#[macro_export]
macro_rules! normal {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Normal::new($value)))
    };
}

/// Create a [`NormalSlice`] array argument.
#[macro_export]
macro_rules! normal_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::NormalSlice::new($value)))
    };
}

/// Create a [`MatrixF32`] row-major, 4×4 transformation matrix argument.
/// The matrix is given as 16 [`f32`] values.
#[macro_export]
macro_rules! matrix_f32 {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::MatrixF32::new($value)))
    };
}

/// Create a [`MatrixF32Slice`] row-major, 4×4 transformation matrices argument.
/// Each matrix is given as 16 [`f32`] values.
#[macro_export]
macro_rules! matrix_f32_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::MatrixF32Slice::new($value)))
    };
}

/// Create a [`MatrixF64`] row-major, 4×4 transformation matrix argument.
/// The matrix is given as 16 [`f64`] values.
///
/// # Examples
///
/// ```
/// # use nsi_ffi_wrap as nsi;
/// # let ctx = nsi::Context::new(None).unwrap();
/// // Setup a transform node.
/// ctx.create("xform", nsi::TRANSFORM, None);
/// ctx.connect("xform", None, nsi::ROOT, "objects", None);
///
/// // Translate 5 units along z-axis.
/// ctx.set_attribute(
///     "xform",
///     &[nsi::matrix_f64!(
///         "transformationmatrix",
///         &[
///             1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.,
///         ]
///     )],
/// );
/// ```
#[macro_export]
macro_rules! matrix_f64 {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::MatrixF64::new($value)))
    };
}

/// Create a [`MatrixF64Slice`] row-major, 4×4 transformation matrices argument.
/// Each matrix is given as 16 [`f64`] values.
#[macro_export]
macro_rules! matrix_f64_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new(
            $name,
            nsi::ArgData::from(nsi::MatrixF64Slice::new($value)),
        )
    };
}

/// Create a [`String`] argument.
///
/// # Examples
///
/// ```
/// # use nsi_ffi_wrap as nsi;
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
///
/// # Examples
///
/// ```
/// # use nsi_ffi_wrap as nsi;
/// # let ctx = nsi::Context::new(None).unwrap();
/// // One of these is not an actor:
/// ctx.set_attribute(
///     "dummy",
///     &[nsi::string_slice!(
///         "actors",
///         &["Klaus Kinski", "Giorgio Moroder", "Rainer Brandt"]
///     )],
/// );
/// ```
#[macro_export]
macro_rules! string_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::StringSlice::new($value)))
    };
}

/// Create a [`Reference`] argument.
///
/// This macro accepts:
/// - `&Box<T>` - ReferenceSlice to boxed data
/// - `&Arc<T>` - ReferenceSlice to Arc'd data  
/// - `&Pin<Box<T>>` - ReferenceSlice to pinned boxes
///
/// For other types with stable addresses, use `reference_stable!` instead.
#[macro_export]
macro_rules! reference {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Reference::new($value)))
    };
}

/// Create a [`Reference`] argument from data with a stable address.
///
/// # Safety
/// You must ensure the data has a stable address (heap allocated, static, etc.)
/// and won't be moved for the lifetime of the reference.
#[macro_export]
macro_rules! reference_stable {
    ($name: tt, $value: expr) => {
        nsi::Arg::new(
            $name,
            nsi::ArgData::from(unsafe { nsi::Reference::from_stable($value) }),
        )
    };
}

/// Create a [`Reference`] array argument.
#[macro_export]
macro_rules! reference_slice {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::ReferenceSlice::new($value)))
    };
}

/// Create a [`Callback`] argument.
#[macro_export]
macro_rules! callback {
    ($name: tt, $value: expr) => {
        nsi::Arg::new($name, nsi::ArgData::from(nsi::Callback::new($value)))
    };
}
