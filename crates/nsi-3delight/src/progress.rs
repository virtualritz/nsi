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
//! let render = reporter.start(&ctx, None);
//! render.wait();
//! ```
//!
//! [`ProgressReporter::start`] borrows the reporter for the whole
//! render, so dropping it mid-render does not compile:
//!
//! ```compile_fail,E0505
//! # use nsi_ffi_wrap as nsi;
//! # use nsi_3delight::progress::{Progress, ProgressCallback, ProgressReporter};
//! # struct Bar;
//! # impl ProgressCallback for Bar { fn update(&self, _: &Progress) {} }
//! let reporter = ProgressReporter::new(Bar);
//! # let ctx = nsi::Context::new(None).unwrap();
//! let render = reporter.start(&ctx, None);
//! drop(reporter);
//! render.wait();
//! ```
//!
//! The reporter must outlive the render -- the renderer holds the
//! pointer until the render stops -- and the compiler already enforces
//! it. `Reference` occupies `ArgData`'s second lifetime, the one pegged
//! to the `Context`, so a reporter borrowed by an argument is borrowed
//! for as long as the context lives, not merely for the
//! `render_control` call. Dropping it mid-render does not compile,
//! whichever of the two entry points you use.
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
use core::{ffi::c_void, panic::AssertUnwindSafe};
use nsi_ffi_wrap as nsi;

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
        let vtable = Box::new(VTable::new([update_trampoline::<C>
            as extern "C" fn(_, _, _)
            as *const c_void]));
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

    /// Starts a render with this reporter attached, and returns a guard
    /// that must be held until the render is done.
    ///
    /// Prefer this to [`arg`](Self::arg) for the ergonomics; both are
    /// sound. Dropping the reporter mid-render is already rejected,
    /// because `Reference` occupies `ArgData`'s second lifetime, the
    /// one pegged to the `Context` -- so the borrow lasts as long as
    /// the context does, not just the `render_control` call:
    ///
    /// ```compile_fail,E0505
    /// # use nsi_ffi_wrap as nsi;
    /// # use nsi_3delight::progress::{Progress, ProgressCallback, ProgressReporter};
    /// # struct Bar;
    /// # impl ProgressCallback for Bar { fn update(&self, _: &Progress) {} }
    /// # let reporter = ProgressReporter::new(Bar);
    /// # let ctx = nsi::Context::new(None).unwrap();
    /// ctx.render_control(nsi::Action::Start, Some(&[reporter.arg()]));
    /// drop(reporter);                                  // <-- nothing stops this
    /// ctx.render_control(nsi::Action::Wait, None);      // renderer calls freed memory
    /// ```
    ///
    /// [`RenderGuard`] is therefore a convenience, not a soundness fix:
    /// one call instead of two, a wait that cannot be forgotten because
    /// `Drop` performs it, and `#[must_use]` so the guard cannot be
    /// discarded by accident.
    ///
    /// ```no_run
    /// # use nsi_ffi_wrap as nsi;
    /// # use nsi_3delight::progress::{Progress, ProgressCallback, ProgressReporter};
    /// # struct Bar;
    /// # impl ProgressCallback for Bar { fn update(&self, _: &Progress) {} }
    /// # let reporter = ProgressReporter::new(Bar);
    /// # let ctx = nsi::Context::new(None).unwrap();
    /// let render = reporter.start(&ctx, None);
    /// render.wait();
    /// ```
    pub fn start<'g, 'a>(
        &'a self,
        ctx: &'g nsi::Context<'a>,
        args: Option<&nsi::ArgSlice<'_, 'a>>,
    ) -> RenderGuard<'g, 'a> {
        let mut all = args.map(<[_]>::to_vec).unwrap_or_default();
        all.push(self.arg());
        ctx.render_control(nsi::Action::Start, Some(&all));
        RenderGuard { ctx }
    }

    /// The `progresscallback` argument to pass to
    /// [`render_control`](nsi_ffi_wrap::Context::render_control).
    ///
    /// The borrow stops the argument outliving the reporter, but it
    /// ends when `render_control` returns, while the renderer holds the
    /// pointer until the render stops. Use [`start`](Self::start)
    /// unless you are assembling the argument list yourself, and then
    /// keep the reporter alive past the wait by hand.
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

/// A render in progress, with a [`ProgressReporter`] attached.
///
/// The reporter is borrowed for `'a`, the lifetime `Context` pegs its
/// references to, so it must outlive the context -- which is exactly
/// the requirement the renderer imposes, now checked. The guard's own
/// borrow of the context is separate and shorter, so the context can
/// still be dropped at the end of its scope.
///
/// Dropping it waits, so a guard that goes out of scope cannot leave
/// the renderer holding a pointer to a reporter that is about to be
/// freed. Call [`wait`](Self::wait) to be explicit, or
/// [`stop`](Self::stop) to end the render early.
#[must_use = "dropping the guard waits for the render; bind it to keep \
              rendering, or call wait() to be explicit"]
pub struct RenderGuard<'r, 'a> {
    ctx: &'r nsi::Context<'a>,
}

impl RenderGuard<'_, '_> {
    /// Blocks until the render finishes.
    pub fn wait(self) {
        // `Drop` does the work, so there is exactly one path.
    }

    /// Stops the render, then waits for it to come to rest.
    pub fn stop(self) {
        self.ctx.render_control(nsi::Action::Stop, None);
        // The `Wait` still happens in `Drop`.
    }
}

impl Drop for RenderGuard<'_, '_> {
    fn drop(&mut self) {
        self.ctx.render_control(nsi::Action::Wait, None);
    }
}
