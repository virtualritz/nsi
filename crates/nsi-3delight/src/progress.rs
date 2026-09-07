//! Render progress reporting.
//!
//! `progresscallback` is a **3Delight extension**, not core ɴsɪ: it has
//! no mention in `nsi.h`, and its interface lives in
//! `$DELIGHT/include/3Delight/Progress.h`. That is why it is here
//! rather than in `nsi-ffi-wrap`, which carries only what `nsi.h`
//! defines -- `stoppedcallback` among it.
//!
//! # Where it goes
//!
//! On [`render_control`](nsi_ffi_wrap::Context::render_control), and
//! nowhere else. Measured, over one scene, three placements:
//!
//! | Placement                  | `update` calls |
//! |----------------------------|----------------|
//! | `Context::new` argument    | 0              |
//! | `.global` attribute        | 0              |
//! | `render_control`           | 306            |
//!
//! ```no_run
//! # use nsi_ffi_wrap as nsi;
//! # use nsi_3delight::progress::{Progress, ProgressCallback, ProgressReporter};
//! # struct Bar;
//! # impl ProgressCallback for Bar {
//! #     fn update(&self, _: &Progress) {}
//! # }
//! // Declared before the context, so it outlives it -- the borrow in
//! // `arg()` makes that a compile error rather than a dangling pointer
//! // if you get it the wrong way round.
//! let reporter = ProgressReporter::new(Bar);
//! # let ctx = nsi::Context::new(None).unwrap();
//!
//! ctx.render_control(nsi::Action::Start, Some(&[reporter.arg()]));
//! ctx.render_control(nsi::Action::Wait, None);
//! ```
//!
//! The reporter must outlive the render. Dropping it while the renderer
//! still holds the pointer would leave it calling into freed memory,
//! so keep it alive until after [`Action::Wait`](nsi_ffi_wrap::Action)
//! returns.
//!
//! # `seconds_rendering` is not elapsed time
//!
//! `Progress::seconds_rendering` is documented in the C++ header as
//! "Number of seconds we've been rendering (excluding init)". It does
//! not behave that way. Observed across one render:
//!
//! ```text
//! first  progress = 0.0250   seconds_rendering = 1788771720.930
//! last   progress = 1.0000   seconds_rendering = 0.001
//! ```
//!
//! The first figure is a Unix epoch timestamp, not a duration.
//! Reproduced twice, on different scenes, and asserted loosely by
//! `tests/progress.rs`, which prints both.
//!
//! During the render it carries the render's **start timestamp**, a
//! Unix epoch value; only on the final call, at `progress == 1.0`, is
//! it an elapsed duration. Anything computing an ETA must take elapsed
//! time from its own clock:
//!
//! ```text
//! eta = elapsed * (1.0 - progress) / progress
//! ```
//!
//! The field is exposed verbatim rather than corrected, because a
//! wrapper cannot tell which of the two meanings a given call carries
//! without guessing at the renderer's clock.

use crate::cpp_object::{CppObject, VTable};
use nsi_ffi_wrap as nsi;
use core::{ffi::c_void, panic::AssertUnwindSafe};

/// A progress report, as `NSI::ProgressCallback::Value`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    /// Overall progress, `0.0` to `1.0`.
    pub render_progress: f64,
    /// **Not** elapsed seconds for most calls -- see the module docs.
    pub seconds_rendering: f64,
    /// Completed rendering passes.
    pub completed_passes: u32,
    /// Total rendering passes.
    pub total_passes: u32,
}

/// Receives progress reports during a render.
///
/// `update` takes `&self`, and the trait requires [`Sync`], because
/// nothing in `Progress.h` promises the renderer serialises the calls
/// and a wrapper cannot make that promise on the renderer's behalf. Use
/// atomics, or a lock, to accumulate state.
///
/// 3Delight 2.9.208 *was* observed to serialise them --
/// `tests/progress.rs` watches for overlapping calls and saw none -- so
/// the bound is conservative rather than forced. It stays because an
/// unpromised observation is not something to make a safety invariant
/// out of: were it `&mut self` and the renderer ever became
/// concurrent, every existing implementation would become unsound
/// silently.
pub trait ProgressCallback: Sync {
    /// Called as the render advances, many times: 20 for a 64x64 frame,
    /// 306 for a larger one. Neither the count nor the spacing is
    /// promised.
    fn update(&self, progress: &Progress);
}

/// `kCurrentVersion` in `Progress.h`.
const CURRENT_VERSION: u32 = 1;

/// The `this`-taking function the renderer calls through vtable slot 0.
///
/// Monomorphised per callback type, so the payload is recovered as a
/// typed `&C` rather than through a type-erased cast.
extern "C" fn update_trampoline<C: ProgressCallback>(
    this: *mut c_void,
    _context: core::ffi::c_int,
    value: *const Progress,
) {
    if this.is_null() || value.is_null() {
        return;
    }
    // A panic must not unwind into C -- the same rule `nsi-ffi-wrap`'s
    // `stoppedcallback` trampoline follows.
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `this` is the `CppObject` we handed the renderer,
        // whose payload is a pointer to the boxed callback that
        // outlives it. `value` is valid for this call only, and is not
        // retained.
        unsafe {
            let object = &*(this as *const CppObject<*const C>);
            let callback = &**object.payload();
            callback.update(&*value);
        }
    }));
}

/// The C++ object 3Delight is handed, wrapping a [`ProgressCallback`].
///
/// Keep it alive for the whole render: the renderer borrows the pointer
/// rather than taking ownership, so dropping this while a render is
/// running leaves it calling into freed memory.
pub struct ProgressReporter<C: ProgressCallback> {
    /// Boxed, so the addresses the object holds stay valid however the
    /// reporter itself is moved.
    object: Box<CppObject<*const C>>,
    #[allow(dead_code)]
    vtable: Box<VTable<1>>,
    callback: Box<C>,
}

impl<C: ProgressCallback> ProgressReporter<C> {
    /// Wraps `callback` in the object layout `Progress.h` describes.
    pub fn new(callback: C) -> Self {
        let callback = Box::new(callback);
        // `NSI::ProgressCallback` declares one pure virtual, `Update`,
        // so the vtable has exactly one slot.
        let vtable = Box::new(VTable::new([
            update_trampoline::<C> as extern "C" fn(_, _, _) as *const c_void,
        ]));
        let object = Box::new(CppObject::new(
            vtable.as_ptr(),
            CURRENT_VERSION,
            &*callback as *const C,
        ));
        Self {
            object,
            vtable,
            callback,
        }
    }

    /// The `progresscallback` argument to pass to
    /// [`render_control`](nsi_ffi_wrap::Context::render_control).
    ///
    /// The borrow is what keeps this sound: the argument cannot outlive
    /// the reporter, so the renderer cannot be handed a pointer to an
    /// object that has already been dropped.
    pub fn arg(&self) -> nsi::Arg<'_, '_> {
        nsi::Arg::new(
            "progresscallback",
            nsi::ArgData::from(nsi::Reference::new(&self.object)),
        )
    }

    /// The callback, for reading whatever it accumulated.
    pub fn callback(&self) -> &C {
        &self.callback
    }
}
