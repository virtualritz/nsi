//! GPU-resident pixel streaming for ɴsɪ.
//!
//! This crate implements the pixel streaming contract specified in
//! `specs/001-gpu-pixel-streaming`: rendered pixels are published into a
//! driver-owned ring of images that a client leases, so an interactive
//! render can be displayed without a copy through the client's CPU memory.
//!
//! # API Conformance
//!
//! The whole surface rides on the **standard `outputdriver` node**. A client
//! creates one with `drivername "nsi-stream"` and sets `stream.*`
//! attributes on it -- nothing else. There is no new ɴsɪ call, no new node
//! type, and no out-of-band handshake (R1). A conforming renderer that does
//! not know this driver simply ignores the attributes, which is exactly what
//! the ɴsɪ specification prescribes for driver-specific attributes.
//!
//! ```no_run
//! # use nsi_stream::{Attr, Extent, Layer, LayerFormat, StreamConfig,
//! #     StreamDriver, StaticProbe};
//! // What the client sets on the `outputdriver` node.
//! let attributes = [
//!     Attr::string("drivername", "nsi-stream"),
//!     Attr::int("stream.version", 1),
//!     Attr::string("stream.transport", "auto"),
//!     Attr::string("stream.publish", "commit"),
//!     Attr::int("stream.ring", 3),
//! ];
//!
//! // What the driver does with them.
//! let (config, warnings) = StreamConfig::parse(&attributes)?;
//! warnings.iter().for_each(|warning| eprintln!("nsi-stream: {warning}"));
//!
//! let layers = vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16)];
//! let driver = StreamDriver::open(
//!     config,
//!     layers,
//!     Extent::new(1920, 1080),
//!     &StaticProbe::for_this_build(),
//! )?;
//!
//! // Renderer side: buckets accumulate, a `synchronize` commits.
//! driver.commit(0)?;
//!
//! // Client side: latest-wins, never blocking.
//! let client = driver.client();
//! if let Some(token) = client.acquire() {
//!     client.wait(token.publication(), None)?;
//!     let _pixels = token.plane(0).expect("the beauty plane");
//!     client.release(token);
//! }
//! # Ok::<(), nsi_stream::Error>(())
//! ```
//!
//! # The Vocabulary
//!
//! Version 1 of the `stream.*` table, frozen (see [`config`] for the full
//! rules and failure modes):
//!
//! | Attribute | ɴsɪ type | Req. | Meaning |
//! | --- | --- | --- | --- |
//! | `stream.version` | `int` | yes | Vocabulary version. Only `1` is supported. |
//! | `stream.transport` | `string` | no | `"auto"` (default), `"gpu"`, `"shm"`, `"callback"`. |
//! | `stream.publish` | `string` | no | `"commit"` (default) or `"continuous"`. |
//! | `stream.ring` | `int` | no | Ring size, default `3`, minimum `2`. |
//! | `stream.channel` | `string` | no | Rendezvous endpoint name (local socket). |
//! | `stream.device.uuid` | `string` | no | Adapter UUID the client renders on. |
//! | `stream.callback.open` | `pointer` | no | In-process open notification. |
//! | `stream.callback.publish` | `pointer` | no | In-process publication notification. |
//! | `stream.callback.close` | `pointer` | no | In-process close notification. |
//! | `stream.onclientloss` | `string` | no | `"continue"` (default) or `"stop"`. |
//!
//! # Lifecycle
//!
//! ```text
//! Configured --open()--------------> Open --first publication--> Streaming
//! Streaming  --resize + commit-----> Resizing --all old released--> Streaming
//! Streaming|Resizing --close()-----> Draining --final timeline value--> Closed
//! open() failure ------------------> Failed (typed error, nothing allocated)
//! ```
//!
//! [`StreamDriver::open`] validates the version, the transport viability,
//! the device UUID, the layer formats and the extent **before** it allocates
//! anything, so a failed open leaves nothing to clean up. Closing signals a
//! final timeline value and drains: outstanding leases stay valid and the
//! stream reaches `Closed` when the last one is released
//! ([`StreamClient::is_drained`]).
//!
//! # Wire Formats
//!
//! Three surfaces cross a process or FFI boundary and are therefore product
//! behavior, frozen at `stream.version` 1 and documented with their
//! compatibility rules in the module that implements them:
//!
//! - the attribute vocabulary -- [`config`],
//! - the shared-memory layout -- [`transport::shm`] (feature `shm`),
//! - the `stream.channel` message framing and handle passing --
//!   [`channel`] (feature `shm`).
//!
//! GPU resources cross the boundary only as exportable OS handles (R2); see
//! [`transport::gpu`] (feature `vulkan`).
//!
//! # Features
//!
//! - `shm` -- the shared-memory transport and the `stream.channel`
//!   rendezvous (Unix).
//! - `vulkan` -- the GPU-resident transport on `ash`.
//! - `delight-bridge` -- the 3Delight bridge ([`bridge`]), which feeds the
//!   ring from the renderer's display-driver callbacks (R10). This is the
//!   one feature that pulls in `nsi-ffi-wrap`.
//!
//! None is on by default: a client that only needs the in-process path
//! takes no graphics, no syscall and no FFI dependencies (R9).

#![deny(missing_docs)]

#[cfg(feature = "delight-bridge")]
pub mod bridge;
#[cfg(all(unix, feature = "shm"))]
pub mod channel;
pub mod config;
pub mod error;
pub mod layer;
pub mod ring;
pub mod timeline;
pub mod transport;

pub use config::{
    Attr, AttrValue, CallbackPointer, CallbackPointers, ClientLoss,
    DRIVER_NAME, NAMESPACE, PublishMode, SUPPORTED_VERSION, StreamConfig,
    Warning,
};
pub use error::{Error, Result};
pub use layer::{Bucket, Extent, Layer, LayerFormat};
pub use ring::{AcquireToken, Publication, PublicationRing, WriteGuard};
pub use timeline::CpuTimeline;
pub use transport::{
    StaticProbe, Transport, TransportProbe, TransportRequest,
    callback::{CallbackTransport, CloseNotice, OpenNotice},
    negotiate,
};

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

// ─── Lifecycle ──────────────────────────────────────────────────────────────

/// Where a stream is in its lifecycle.
///
/// `Failed` never appears on a live [`StreamDriver`]: a failed open returns
/// a typed [`Error`] instead of a half-built driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamState {
    /// The vocabulary decoded, nothing allocated yet.
    Configured,
    /// Open, ring allocated, nothing published yet.
    Open,
    /// At least one publication was announced.
    Streaming,
    /// Reallocated at a new extent; pre-resize leases may still be out.
    Resizing,
    /// Closed, waiting for the last lease to come back.
    Draining,
    /// Closed and drained.
    Closed,
    /// Open failed. Reported as an [`Error`], never as driver state.
    Failed,
}

// ─── StreamDriver ───────────────────────────────────────────────────────────

/// The renderer-side end of a stream.
///
/// Owns the publication ring, the timeline and the notification sink. A
/// renderer (or the bridge standing in for one) writes buckets and commits;
/// everything else is the client's.
#[derive(Debug)]
pub struct StreamDriver {
    config: StreamConfig,
    transport: Transport,
    ring: Arc<PublicationRing>,
    callbacks: Mutex<CallbackTransport>,
    state: Mutex<StreamState>,
}

impl StreamDriver {
    /// Validate everything, then open the stream.
    ///
    /// The order matters and is contractual: the version is checked by
    /// [`StreamConfig::parse`], then the layer set and extent, then the
    /// transport is negotiated -- and only then is memory allocated. A
    /// failure leaves nothing to clean up (`data-model.md`, lifecycle
    /// rules).
    ///
    /// # Errors
    ///
    /// - [`Error::UnsupportedVersion`] -- the config carries a version this
    ///   build does not implement.
    /// - [`Error::MalformedAttribute`] -- a `stream.*` value that cannot be
    ///   honored, e.g. `stream.transport "shm"` without `stream.channel`.
    /// - [`Error::TransportUnavailable`], [`Error::DeviceMismatch`] -- see
    ///   [`transport::negotiate`].
    /// - [`Error::InvalidWrite`] -- no layer connected, or a degenerate
    ///   extent.
    pub fn open(
        config: StreamConfig,
        layers: Vec<Layer>,
        extent: Extent,
        probe: &dyn TransportProbe,
    ) -> Result<Self> {
        if SUPPORTED_VERSION != config.version {
            Err(Error::UnsupportedVersion {
                requested: config.version,
                supported: SUPPORTED_VERSION,
            })?;
        }
        if layers.is_empty() {
            Err(Error::invalid_write(
                "at least one `outputlayer` must be connected",
            ))?;
        }
        if extent.is_empty() {
            Err(Error::invalid_write(format!("degenerate extent {extent}")))?;
        }
        if config.ring < StreamConfig::MIN_RING {
            Err(Error::malformed(
                "stream.ring",
                format!(
                    "ring size must be at least {}, got {}",
                    StreamConfig::MIN_RING,
                    config.ring
                ),
            ))?;
        }

        let transport = negotiate(&config, probe)?;

        // Everything is validated -- allocate.
        let ring = Arc::new(PublicationRing::new(
            layers.clone(),
            extent,
            config.ring,
            config.publish,
        )?);

        let mut callbacks =
            CallbackTransport::new().with_pointers(config.callbacks);
        callbacks.notify_open(&OpenNotice {
            transport,
            extent,
            layers,
            ring: config.ring,
        });

        Ok(Self {
            config,
            transport,
            ring,
            callbacks: Mutex::new(callbacks),
            state: Mutex::new(StreamState::Open),
        })
    }

    /// Install the in-process notification sink.
    ///
    /// Replaces whatever was there; the [`OpenNotice`] has already been
    /// delivered by then, so a sink installed here sees publications and the
    /// close only.
    pub fn set_callbacks(&self, callbacks: CallbackTransport) {
        *self.callbacks.lock().expect("callback mutex") = callbacks;
    }

    /// The decoded configuration.
    #[inline]
    pub const fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// The negotiated transport.
    #[inline]
    pub const fn transport(&self) -> Transport {
        self.transport
    }

    /// The publication ring.
    #[inline]
    pub const fn ring(&self) -> &Arc<PublicationRing> {
        &self.ring
    }

    /// Current lifecycle state.
    pub fn state(&self) -> StreamState {
        let state = *self.state.lock().expect("state mutex");

        if StreamState::Draining == state && self.ring.is_drained() {
            StreamState::Closed
        } else {
            state
        }
    }

    /// A client facade on the same ring (in-process path).
    pub fn client(&self) -> StreamClient {
        StreamClient {
            ring: Arc::clone(&self.ring),
        }
    }

    /// Copy a rendered bucket into the accumulation buffer.
    ///
    /// # Errors
    ///
    /// See [`PublicationRing::write_bucket`].
    pub fn write_bucket(
        &self,
        layer: usize,
        bucket: Bucket,
        data: &[u8],
    ) -> Result<()> {
        self.ring.write_bucket(layer, bucket, data)
    }

    /// Publish the accumulation for scene generation `generation` -- the
    /// driver's response to an applied `synchronize`.
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] once the stream is closed.
    pub fn commit(&self, generation: u64) -> Result<Option<Publication>> {
        let published = self.ring.commit(generation)?;
        self.after_publish(published);

        Ok(published)
    }

    /// Publish progressive refinement. A no-op in
    /// [`PublishMode::Commit`].
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] once the stream is closed.
    pub fn publish_progressive(&self) -> Result<Option<Publication>> {
        let published = self.ring.publish_progressive()?;
        self.after_publish(published);

        Ok(published)
    }

    /// Reallocate the ring at a new extent (a `screen` resolution edit
    /// followed by `synchronize`).
    ///
    /// # Errors
    ///
    /// See [`PublicationRing::resize`].
    pub fn resize(&self, extent: Extent) -> Result<()> {
        self.ring.resize(extent)?;
        *self.state.lock().expect("state mutex") = StreamState::Resizing;

        Ok(())
    }

    /// Stop publishing, signal the final timeline value and start draining.
    ///
    /// Returns the final timeline value. Idempotent.
    pub fn close(&self) -> u64 {
        let final_value = self.ring.close();

        *self.state.lock().expect("state mutex") = StreamState::Draining;

        self.callbacks.lock().expect("callback mutex").notify_close(
            &CloseNotice {
                final_timeline_value: final_value,
                published: self.ring.published(),
                dropped: self.ring.dropped(),
            },
        );

        final_value
    }

    fn after_publish(&self, published: Option<Publication>) {
        if let Some(publication) = published {
            let mut state = self.state.lock().expect("state mutex");

            if StreamState::Open == *state || StreamState::Resizing == *state {
                *state = StreamState::Streaming;
            }
            drop(state);

            self.callbacks
                .lock()
                .expect("callback mutex")
                .notify_publish(&publication);
        }
    }
}

// ─── StreamClient ───────────────────────────────────────────────────────────

/// The client-side end of a stream.
///
/// Identical in shape for every transport (US3): acquire the latest
/// publication, wait on its timeline value, sample, release.
#[derive(Debug, Clone)]
pub struct StreamClient {
    ring: Arc<PublicationRing>,
}

impl StreamClient {
    /// Build a client on an existing ring.
    pub fn new(ring: Arc<PublicationRing>) -> Self {
        Self { ring }
    }

    /// Take a lease on the latest publication.
    ///
    /// Never blocks on renderer progress; `None` means "nothing new".
    pub fn acquire(&self) -> Option<AcquireToken> {
        self.ring.acquire()
    }

    /// Wait until a publication's contents are complete.
    ///
    /// # Errors
    ///
    /// [`Error::WaitTimeout`] carrying the publication's serial when
    /// `timeout` expires first. The client must not spin instead.
    pub fn wait(
        &self,
        publication: &Publication,
        timeout: Option<Duration>,
    ) -> Result<()> {
        self.ring
            .timeline()
            .wait(publication.timeline_value, timeout)
    }

    /// Return a lease.
    pub fn release(&self, token: AcquireToken) {
        self.ring.release(token);
    }

    /// The ring this client reads from.
    #[inline]
    pub const fn ring(&self) -> &Arc<PublicationRing> {
        &self.ring
    }

    /// Whether the stream is closed and every lease was returned.
    pub fn is_drained(&self) -> bool {
        self.ring.is_drained()
    }
}
