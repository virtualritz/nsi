//! Synthesising the C++ objects 3Delight's extension headers ask for.
//!
//! Several 3Delight extensions take a pointer to an object deriving
//! from an abstract C++ class -- `NSI::ProgressCallback` in
//! `3Delight/Progress.h`, `DlOceanEvaluator` in `3Delight/OceanBake.h`.
//! They share a house shape: a `unsigned m_version{kCurrentVersion}`
//! field and one or more pure virtuals. Rust cannot derive from a C++
//! class, so we build the object the Itanium C++ ABI describes by hand.
//!
//! # The layout, and the one thing that is easy to get wrong
//!
//! An object with virtuals begins with a pointer to its vtable, and a
//! vtable is laid out as
//!
//! ```text
//!   [-2] offset-to-top
//!   [-1] typeinfo pointer
//!   [ 0] &first virtual        <- the object's vptr points HERE
//!   [ 1] &second virtual
//! ```
//!
//! The indices are the giveaway: the vptr addresses the **first
//! function slot**, not the start of the allocation. Point it at the
//! start and the first virtual call jumps through `offset-to-top`,
//! which is zero -- a jump to address 0. [`VTable`] gets this right by
//! construction, because `slots` is its own field and `as_ptr` returns
//! the address of *that*.
//!
//! Neither class here declares a virtual destructor, so there are no
//! destructor slots ahead of the functions, and the renderer never
//! deletes through the pointer -- it is borrowed for the call.
//!
//! `typeinfo` is null. It is only read by `dynamic_cast`, `typeid` and
//! exception matching, none of which the renderer performs on a
//! callback it was handed.

use core::ffi::c_void;

/// A C++ vtable with `SLOTS` virtual functions.
///
/// Build it with [`VTable::new`] and hand [`VTable::as_ptr`] to
/// [`CppObject::new`].
#[repr(C)]
pub struct VTable<const SLOTS: usize> {
    /// Distance from this subobject to the top of the complete object.
    /// Zero: there is no multiple inheritance here.
    offset_to_top: isize,
    /// Null; see the module docs for why that is sound here.
    type_info: *const c_void,
    /// The function pointers, in declaration order.
    slots: [*const c_void; SLOTS],
}

// SAFETY: the vtable is immutable once built, and every pointer in it
// is a `'static` function pointer.
unsafe impl<const SLOTS: usize> Sync for VTable<SLOTS> {}

impl<const SLOTS: usize> VTable<SLOTS> {
    /// A vtable whose slots are `functions`, in the order the C++ class
    /// declares its virtuals.
    ///
    /// Each entry must be a pointer to an `extern "C"` function whose
    /// first parameter is the `this` pointer -- the implicit argument a
    /// C++ member function takes and Rust does not.
    pub const fn new(functions: [*const c_void; SLOTS]) -> Self {
        Self {
            offset_to_top: 0,
            type_info: core::ptr::null(),
            slots: functions,
        }
    }

    /// The address a conforming object's vptr must hold: the first
    /// function slot, *not* the start of the vtable.
    pub const fn as_ptr(&self) -> *const *const c_void {
        self.slots.as_ptr()
    }
}

/// An object laid out as 3Delight's abstract callback classes are: a
/// vtable pointer, the `m_version` they all carry, and whatever state
/// the implementation needs behind it.
///
/// The renderer reads only the first two fields. `payload` sits after
/// them, where C++ would have put a derived class's members, and is
/// reached from a virtual by casting `this` back to this type.
#[repr(C)]
pub struct CppObject<T> {
    vptr: *const *const c_void,
    version: u32,
    /// Explicit, so the payload's offset does not depend on the
    /// alignment of `T`.
    _padding: u32,
    payload: T,
}

impl<T> CppObject<T> {
    /// Builds the object. `vtable` must outlive it -- in practice a
    /// `static`.
    pub fn new(vtable: *const *const c_void, version: u32, payload: T) -> Self {
        Self {
            vptr: vtable,
            version,
            _padding: 0,
            payload,
        }
    }

    /// The payload, for a virtual that has cast `this` back.
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// The pointer to hand the renderer.
    pub fn as_ptr(&self) -> *const c_void {
        self as *const Self as *const c_void
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vptr must address the first *function* slot. Getting this
    /// wrong points it at `offset_to_top`, which is zero, so the first
    /// virtual call jumps to address 0.
    #[test]
    fn the_vptr_addresses_the_first_function_slot() {
        static VTABLE: VTable<2> =
            VTable::new([0x1111 as *const c_void, 0x2222 as *const c_void]);

        let base = &VTABLE as *const VTable<2> as usize;
        let vptr = VTABLE.as_ptr() as usize;

        assert_eq!(
            2 * size_of::<usize>(),
            vptr - base,
            "the vptr must skip offset-to-top and typeinfo"
        );
        // SAFETY: `vptr` addresses `slots`, which holds two entries.
        unsafe {
            assert_eq!(0x1111, *(vptr as *const *const c_void) as usize);
            assert_eq!(0x2222, *(vptr as *const *const c_void).add(1) as usize);
        }
    }

    /// The renderer reads `m_version` at offset 8, straight after the
    /// vptr. C++ initialises it with a default member initialiser,
    /// which does not exist for an object we lay out ourselves, so it
    /// has to be written explicitly -- and land in the right place.
    #[test]
    fn version_sits_directly_after_the_vptr() {
        static VTABLE: VTable<1> = VTable::new([core::ptr::null()]);
        let object = CppObject::new(VTABLE.as_ptr(), 1, 42u64);

        let base = &object as *const CppObject<u64> as usize;
        let version = &object.version as *const u32 as usize;

        assert_eq!(size_of::<usize>(), version - base, "m_version at offset 8");
        assert_eq!(1, object.version);
        assert_eq!(&42, object.payload());
    }
}
