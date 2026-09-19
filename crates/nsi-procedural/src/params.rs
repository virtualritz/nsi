//! A borrowed view over the parameters a procedural is executed with.

use core::{
    ffi::{CStr, c_char, c_int, c_void},
    slice,
};
use nsi_ffi_wrap::{FfiParam, Flags, Type};

/// The parameters of one `execute` call: the extra arguments to
/// `NSIEvaluate`, or the extra attributes on a `procedural` node.
///
/// Borrowed, never owned: the array belongs to the renderer and is valid
/// only for the duration of the call. Copy anything you need to keep.
#[derive(Copy, Clone, Debug)]
pub struct Params<'a> {
    raw: &'a [FfiParam],
}

impl<'a> Params<'a> {
    /// # Safety
    ///
    /// `raw` must point to `count` valid `NSIParam_t`s that outlive `'a`,
    /// and every name and data pointer in them must be valid for `'a` and
    /// agree with the parameter's type, count, array length and flags --
    /// all of which ɴsɪ guarantees for the duration of a call.
    pub unsafe fn from_raw(raw: *const FfiParam, count: c_int) -> Self {
        let raw = if raw.is_null() || count <= 0 {
            &[][..]
        } else {
            // SAFETY: the caller guarantees `count` valid entries.
            unsafe { slice::from_raw_parts(raw, count as usize) }
        };
        Self { raw }
    }

    /// How many parameters there are.
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// Every parameter, in the order the renderer passed them.
    pub fn iter(&self) -> impl Iterator<Item = Param<'a>> + use<'a> {
        self.raw.iter().map(|raw| Param { raw })
    }

    /// The first parameter called `name`.
    pub fn get(&self, name: &str) -> Option<Param<'a>> {
        self.iter().find(|param| param.name() == Some(name))
    }
}

/// One parameter.
///
/// The typed accessors return `None` when the parameter is of another
/// type, so asking for the wrong one is an ordinary miss rather than a
/// reinterpretation of its bytes.
#[derive(Copy, Clone, Debug)]
pub struct Param<'a> {
    raw: &'a FfiParam,
}

impl<'a> Param<'a> {
    /// The parameter's name, if it is UTF-8. ɴsɪ names are identifiers,
    /// so in practice always.
    pub fn name(&self) -> Option<&'a str> {
        if self.raw.name.is_null() {
            return None;
        }
        // SAFETY: a non-null name is a NUL-terminated C string valid for
        // `'a`, per `Params::from_raw`.
        unsafe { CStr::from_ptr(self.raw.name) }.to_str().ok()
    }

    /// The parameter's type, or `None` for a type this crate does not
    /// know.
    pub fn type_tag(&self) -> Option<Type> {
        Some(match self.raw.type_ {
            1 => Type::RealF32,
            0x11 => Type::RealF64,
            2 => Type::IntegerI32,
            0x12 => Type::IntegerI64,
            3 => Type::String,
            4 => Type::Color3F32,
            5 => Type::Point3F32,
            6 => Type::Vector3F32,
            7 => Type::Normal3F32,
            8 => Type::Matrix4F32,
            0x18 => Type::Matrix4F64,
            9 => Type::Reference,
            _ => return None,
        })
    }

    /// The parameter's flags. Bits this crate does not know are dropped.
    pub fn flags(&self) -> Flags {
        Flags::from_bits_truncate(self.raw.flags)
    }

    /// How many values the parameter holds -- one per vertex, say, for
    /// a per-vertex attribute.
    pub fn count(&self) -> usize {
        self.raw.count
    }

    /// How many elements make up one value: the array length for an
    /// array type, otherwise 1.
    pub fn array_length(&self) -> usize {
        if self.flags().contains(Flags::IS_ARRAY) {
            self.raw.arraylength.max(0) as usize
        } else {
            1
        }
    }

    /// The scalars of `T` a value of this type is made of: 3 for a
    /// color, 16 for a matrix.
    fn components(type_tag: Type) -> usize {
        match type_tag {
            Type::Color3F32 | Type::Point3F32 | Type::Vector3F32 | Type::Normal3F32 => 3,
            Type::Matrix4F32 | Type::Matrix4F64 => 16,
            _ => 1,
        }
    }

    /// The data as `T`, when the type is one of `types`.
    fn slice<T>(&self, types: &[Type]) -> Option<&'a [T]> {
        let type_tag = self.type_tag()?;
        if !types.contains(&type_tag) {
            return None;
        }
        let len =
            self.count() * self.array_length() * Self::components(type_tag);
        if 0 == len {
            return Some(&[]);
        }
        if self.raw.data.is_null() {
            return None;
        }
        // SAFETY: the type was checked above, and `Params::from_raw`
        // guarantees the data holds `count * array length` values of it,
        // each of `components` scalars, valid for `'a`.
        Some(unsafe { slice::from_raw_parts(self.raw.data as *const T, len) })
    }

    /// The scalars of a `float`, `color`, `point`, `vector`, `normal` or
    /// `matrix` parameter.
    pub fn f32s(&self) -> Option<&'a [f32]> {
        self.slice(&[
            Type::RealF32,
            Type::Color3F32,
            Type::Point3F32,
            Type::Vector3F32,
            Type::Normal3F32,
            Type::Matrix4F32,
        ])
    }

    /// The scalars of a `double` or `doublematrix` parameter.
    pub fn f64s(&self) -> Option<&'a [f64]> {
        self.slice(&[Type::RealF64, Type::Matrix4F64])
    }

    /// The values of an `int` parameter.
    pub fn i32s(&self) -> Option<&'a [i32]> {
        self.slice(&[Type::IntegerI32])
    }

    /// The values of an `int64` parameter.
    pub fn i64s(&self) -> Option<&'a [i64]> {
        self.slice(&[Type::IntegerI64])
    }

    /// The values of a `pointer` parameter.
    pub fn pointers(&self) -> Option<&'a [*const c_void]> {
        self.slice(&[Type::Reference])
    }

    /// The values of a `string` parameter.
    ///
    /// Bytes, not `str`: an ɴsɪ string is whatever the scene wrote, and a
    /// file name need not be UTF-8. A null entry reads as empty.
    pub fn strings(&self) -> Option<impl Iterator<Item = &'a CStr> + use<'a>> {
        let pointers: &'a [*const c_char] = self.slice(&[Type::String])?;
        Some(pointers.iter().map(|&pointer| {
            if pointer.is_null() {
                c""
            } else {
                // SAFETY: a non-null string value is a NUL-terminated C
                // string valid for `'a`, per `Params::from_raw`.
                unsafe { CStr::from_ptr(pointer) }
            }
        }))
    }

    /// The first value of a scalar `float` parameter.
    pub fn f32(&self) -> Option<f32> {
        self.slice(&[Type::RealF32])?.first().copied()
    }

    /// The first value of a scalar `double` parameter.
    pub fn f64(&self) -> Option<f64> {
        self.f64s()?.first().copied()
    }

    /// The first value of an `int` parameter.
    pub fn i32(&self) -> Option<i32> {
        self.i32s()?.first().copied()
    }

    /// The first value of a `string` parameter.
    pub fn string(&self) -> Option<&'a CStr> {
        self.strings()?.next()
    }
}
