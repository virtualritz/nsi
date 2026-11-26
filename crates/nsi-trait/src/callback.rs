//! The callbacks a host hands to a renderer: the error handler and the
//! render-stopped callback.
//!
//! These are the Rust forms of `NSIErrorHandler_t` and
//! `NSIRenderStopped_t` of `nsi.h`. A pure-Rust backend receives them
//! through an argument (`nsi-argument`'s `Callback`) and calls them
//! directly. `nsi-ffi-wrap` converts them at the C boundary.

/// How a render stopped -- `NSIStoppingStatus` of `nsi.h`.
///
/// A renderer reports it to the [`StatusCallback`] of a
/// `render_control` call.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "num_enum", derive(num_enum::FromPrimitive))]
pub enum RenderStatus {
    /// `NSIRenderCompleted`: the render finished.
    #[cfg_attr(feature = "num_enum", num_enum(default))]
    Completed = 0,
    /// `NSIRenderAborted`: the render stopped before it finished, through
    /// a `stop` action or an error.
    Aborted = 1,
    /// `NSIRenderSynchronized`: an interactive render took the edits of a
    /// `synchronize` action.
    Synchronized = 2,
    /// `NSIRenderRestarted`: an interactive render started again after a
    /// `synchronize` action.
    Restarted = 3,
}

/// An unknown value is [`RenderStatus::Completed`], as `nsi-ffi-wrap` has
/// always read it.
#[cfg(not(feature = "num_enum"))]
impl From<i32> for RenderStatus {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Aborted,
            2 => Self::Synchronized,
            3 => Self::Restarted,
            _ => Self::Completed,
        }
    }
}

impl From<RenderStatus> for i32 {
    fn from(status: RenderStatus) -> Self {
        status as _
    }
}

/// A closure that a renderer calls with each error, warning or message,
/// during the scene definition or a render.
///
/// The host passes it as the `"errorhandler"` argument when it creates a
/// context. The arguments are the level, the message id and the message.
/// The levels of `nsi.h` map to [`log::Level`] as `nsi-ffi-wrap` maps
/// them: `NSIErrMessage` is [`Trace`](log::Level::Trace), `NSIErrInfo`
/// is [`Info`](log::Level::Info), `NSIErrWarning` is
/// [`Warn`](log::Level::Warn) and `NSIErrError` is
/// [`Error`](log::Level::Error).
///
/// # Examples
///
/// ```
/// use nsi_trait::ErrorCallback;
///
/// let error_handler = ErrorCallback::new(
///     |level: log::Level, message_id: i32, message: &str| {
///         eprintln!("{level} [{message_id}] {message}");
///     },
/// );
/// error_handler.call(log::Level::Error, 0, "no camera");
/// ```
pub trait FnError<'a>: Fn(
    // The error level.
    log::Level,
    // The message id.
    i32,
    // The message.
    &str,
)
+ 'a {}

#[doc(hidden)]
impl<
    'a,
    T: Fn(log::Level, i32, &str) + 'a + for<'r> Fn(log::Level, i32, &'r str),
> FnError<'a> for T
{
}

/// Wrapper to pass a [`FnError`] closure to a renderer.
pub struct ErrorCallback<'a>(Box<Box<dyn FnError<'a>>>);

// SAFETY: moved unchanged from `nsi-ffi-wrap`, where a renderer calls the
// error handler from its own threads. The host is responsible for a
// closure that tolerates that.
unsafe impl Send for ErrorCallback<'static> {}
unsafe impl Sync for ErrorCallback<'static> {}

impl<'a> ErrorCallback<'a> {
    pub fn new<F>(fn_error: F) -> Self
    where
        F: FnError<'a>,
    {
        ErrorCallback(Box::new(Box::new(fn_error)))
    }

    /// Calls the closure.
    #[inline]
    pub fn call(&self, level: log::Level, message_id: i32, message: &str) {
        (self.0)(level, message_id, message)
    }

    /// The boxed closure as a raw pointer, for a type-erased argument.
    /// `nsi-argument`'s `CallbackPtr` takes it back.
    #[doc(hidden)]
    pub fn into_raw(self) -> *mut Box<dyn FnError<'a>> {
        Box::into_raw(self.0)
    }
}

/// A closure that a renderer calls when a render stops, with the context
/// of the render and the [`RenderStatus`].
///
/// The host passes it as the `"stoppedcallback"` argument of a
/// `render_control` call (`"callback"` in `nsi-ffi-wrap`). `C` is the
/// context type of the backend: a backend whose `Self` is the context
/// passes `&Self`.
///
/// # Examples
///
/// ```
/// use nsi_trait::{RenderStatus, StatusCallback};
///
/// struct Renderer;
///
/// let status_callback =
///     StatusCallback::new(|_renderer: &Renderer, status: RenderStatus| {
///         eprintln!("render stopped: {status:?}");
///     });
/// status_callback.call(&Renderer, RenderStatus::Completed);
/// ```
pub trait FnStatus<'a, C: ?Sized>: Fn(&C, RenderStatus) + 'a {}

#[doc(hidden)]
impl<'a, C: ?Sized, T: Fn(&C, RenderStatus) + 'a> FnStatus<'a, C> for T {}

/// Wrapper to pass a [`FnStatus`] closure to a renderer.
pub struct StatusCallback<'a, C: ?Sized>(Box<Box<dyn FnStatus<'a, C>>>);

// SAFETY: as for `ErrorCallback`.
unsafe impl<C: ?Sized> Send for StatusCallback<'static, C> {}
unsafe impl<C: ?Sized> Sync for StatusCallback<'static, C> {}

impl<'a, C: ?Sized> StatusCallback<'a, C> {
    pub fn new<F>(fn_status: F) -> Self
    where
        F: FnStatus<'a, C>,
    {
        StatusCallback(Box::new(Box::new(fn_status)))
    }

    /// Calls the closure.
    #[inline]
    pub fn call(&self, context: &C, status: RenderStatus) {
        (self.0)(context, status)
    }

    /// The boxed closure as a raw pointer, for a type-erased argument.
    /// `nsi-argument`'s `CallbackPtr` takes it back.
    #[doc(hidden)]
    pub fn into_raw(self) -> *mut Box<dyn FnStatus<'a, C>> {
        Box::into_raw(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn render_status_values_match_the_c_header() {
        // `enum NSIStoppingStatus` of `nsi.h`.
        assert_eq!(i32::from(RenderStatus::Completed), 0);
        assert_eq!(i32::from(RenderStatus::Aborted), 1);
        assert_eq!(i32::from(RenderStatus::Synchronized), 2);
        assert_eq!(i32::from(RenderStatus::Restarted), 3);
    }

    #[test]
    fn render_status_reads_back_and_defaults_to_completed() {
        for status in [
            RenderStatus::Completed,
            RenderStatus::Aborted,
            RenderStatus::Synchronized,
            RenderStatus::Restarted,
        ] {
            assert_eq!(RenderStatus::from(i32::from(status)), status);
        }
        assert_eq!(RenderStatus::from(-1), RenderStatus::Completed);
        assert_eq!(RenderStatus::from(4), RenderStatus::Completed);
    }

    #[test]
    fn an_error_callback_calls_its_closure() {
        let seen = RefCell::new(Vec::new());
        let error_handler =
            ErrorCallback::new(|level: log::Level, id: i32, message: &str| {
                seen.borrow_mut().push((level, id, message.to_string()));
            });
        error_handler.call(log::Level::Warn, 7, "late");
        drop(error_handler);
        assert_eq!(
            seen.into_inner(),
            [(log::Level::Warn, 7, "late".to_string())]
        );
    }

    #[test]
    fn a_status_callback_gets_its_context_and_status() {
        struct Renderer(u32);
        let seen = Cell::new(None);
        let status_callback =
            StatusCallback::new(|renderer: &Renderer, status: RenderStatus| {
                seen.set(Some((renderer.0, status)));
            });
        status_callback.call(&Renderer(3), RenderStatus::Aborted);
        drop(status_callback);
        assert_eq!(seen.get(), Some((3, RenderStatus::Aborted)));
    }
}
