//! In-process callback transport.
//!
//! This is the Rust counterpart of the three `stream.callback.*` pointer
//! attributes: typed closures the driver invokes for open, publish and close
//! notifications. Pointer-typed attributes (and therefore closures) are legal
//! only here -- every out-of-process transport carries exportable OS handles
//! instead (R2).
//!
//! # Direction
//!
//! Notifications flow driver → client, which is the only reverse-direction
//! flow the contract allows: the client never calls into the driver, it only
//! ever *sets attributes* (`data-model.md`, "Direction"). Pixels are not
//! passed to the callbacks -- a notification says *that* something was
//! published, the client then acquires from the ring.
//!
//! # FFI
//!
//! Turning the `stream.callback.*` [`CallbackPointer`]s into the closures
//! below is the job of the renderer bridge (a `Reference`-style trampoline,
//! exactly as `nsi`'s existing `callback!`/`reference!` macros do it). That
//! glue arrives with the 3Delight bridge; this module is the in-process,
//! renderer-independent half and is fully usable on its own.

use crate::{
    config::{CallbackPointer, CallbackPointers},
    layer::{Extent, Layer},
    ring::Publication,
    transport::Transport,
};

/// What the client learns when the stream opens.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpenNotice {
    /// The negotiated transport.
    pub transport: Transport,
    /// Extent of the first ring allocation.
    pub extent: Extent,
    /// Connected layers, in publication plane order.
    pub layers: Vec<Layer>,
    /// Number of ring slots.
    pub ring: usize,
}

/// What the client learns when the stream closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloseNotice {
    /// The final timeline value; nothing will be signaled beyond it.
    pub final_timeline_value: u64,
    /// Publications announced over the lifetime of the stream.
    pub published: u64,
    /// Publications dropped because the ring was fully leased.
    pub dropped: u64,
}

/// Boxed open notification.
pub type OpenCallback = Box<dyn FnMut(&OpenNotice) + Send>;
/// Boxed publication notification.
pub type PublishCallback = Box<dyn FnMut(&Publication) + Send>;
/// Boxed close notification.
pub type CloseCallback = Box<dyn FnMut(&CloseNotice) + Send>;

/// The in-process notification sink.
///
/// All three callbacks are optional; a stream with none is perfectly legal
/// and simply publishes into the ring for a client that polls
/// [`acquire`](crate::ring::PublicationRing::acquire).
#[derive(Default)]
pub struct CallbackTransport {
    open: Option<OpenCallback>,
    publish: Option<PublishCallback>,
    close: Option<CloseCallback>,
    /// The raw `stream.callback.*` attributes this sink was built from, kept
    /// for diagnostics and for the bridge that installs the trampolines.
    pointers: [Option<CallbackPointer>; 3],
}

impl core::fmt::Debug for CallbackTransport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CallbackTransport")
            .field("open", &self.open.is_some())
            .field("publish", &self.publish.is_some())
            .field("close", &self.close.is_some())
            .field("pointers", &self.pointers)
            .finish()
    }
}

impl CallbackTransport {
    /// A sink with no callbacks installed.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install the open notification.
    #[must_use]
    pub fn on_open(
        mut self,
        callback: impl FnMut(&OpenNotice) + Send + 'static,
    ) -> Self {
        self.open = Some(Box::new(callback));
        self
    }

    /// Install the publication notification.
    #[must_use]
    pub fn on_publish(
        mut self,
        callback: impl FnMut(&Publication) + Send + 'static,
    ) -> Self {
        self.publish = Some(Box::new(callback));
        self
    }

    /// Install the close notification.
    #[must_use]
    pub fn on_close(
        mut self,
        callback: impl FnMut(&CloseNotice) + Send + 'static,
    ) -> Self {
        self.close = Some(Box::new(callback));
        self
    }

    /// Record the raw `stream.callback.*` pointers this sink stands for.
    ///
    /// The pointers are never dereferenced here; see the module
    /// documentation.
    #[must_use]
    pub fn with_pointers(mut self, pointers: CallbackPointers) -> Self {
        self.pointers = [pointers.open, pointers.publish, pointers.close];
        self
    }

    /// The recorded `stream.callback.*` pointers, in open/publish/close
    /// order.
    #[inline]
    pub const fn pointers(&self) -> &[Option<CallbackPointer>; 3] {
        &self.pointers
    }

    /// Deliver an open notification.
    pub fn notify_open(&mut self, notice: &OpenNotice) {
        if let Some(callback) = self.open.as_mut() {
            callback(notice);
        }
    }

    /// Deliver a publication notification.
    pub fn notify_publish(&mut self, publication: &Publication) {
        if let Some(callback) = self.publish.as_mut() {
            callback(publication);
        }
    }

    /// Deliver a close notification.
    pub fn notify_close(&mut self, notice: &CloseNotice) {
        if let Some(callback) = self.close.as_mut() {
            callback(notice);
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    #[test]
    fn publish_notifications_are_delivered() {
        let seen = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&seen);

        let mut transport =
            CallbackTransport::new().on_publish(move |publication| {
                counter.store(publication.frame_serial, Ordering::Release);
            });

        transport.notify_publish(&Publication {
            image_index: 1,
            frame_serial: 42,
            scene_generation: 7,
            timeline_value: 42,
            extent: Extent::new(2, 2),
        });

        assert_eq!(seen.load(Ordering::Acquire), 42);
    }
}
