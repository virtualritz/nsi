//! The 3Delight bridge (feature `delight-bridge`).
//!
//! R10: the contract must be testable against a real renderer from day one.
//! 3Delight is today's only ɴsɪ implementation, and the way it hands pixels
//! to a driver is ndspy -- a historical RenderMan artifact that is *not* part
//! of ɴsɪ and never appears in this crate's public surface (`spec.md`,
//! non-goals: "ndspy as a public API [...] may appear only inside a 3Delight
//! bridge implementation detail"). This module is that implementation
//! detail.
//!
//! # What The Bridge Is
//!
//! [`DelightBridge`] is a [`StreamDriver`] plus the three ndspy closures that
//! feed it. The integrator creates a perfectly ordinary `outputdriver` node,
//! points `drivername` at the in-process
//! [`FERRIS_F32`](nsi_ffi_wrap::output::FERRIS_F32) display driver of
//! `nsi-ffi-wrap`, and attaches the bridge's callbacks as the usual
//! `callback.open` / `callback.write` / `callback.finish` pointer
//! attributes:
//!
//! ```no_run
//! # use nsi_ffi_wrap as nsi;
//! # use nsi_stream::{
//! #     Extent, Layer, LayerFormat, StreamConfig, bridge::DelightBridge,
//! # };
//! let bridge = DelightBridge::new(
//!     StreamConfig::default(),
//!     vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32)],
//!     Extent::new(1920, 1080),
//! )?;
//!
//! # let ctx = nsi::Context::new(None).unwrap();
//! ctx.set_attribute(
//!     "driver",
//!     &[
//!         nsi::string!("drivername", nsi::output::FERRIS_F32),
//!         nsi::string!("imagefilename", "stream"),
//!         nsi::callback!("callback.open", bridge.open_callback()),
//!         nsi::callback!("callback.write", bridge.write_callback()),
//!         nsi::callback!("callback.finish", bridge.finish_callback()),
//!     ],
//! );
//!
//! // The client side is the ordinary one -- acquire, sample, release.
//! let client = bridge.client();
//! # Ok::<(), nsi_stream::Error>(())
//! ```
//!
//! Everything downstream of the bucket upload -- ring, latest-wins drops,
//! timeline, resize, close/drain -- is the machinery of [`crate::ring`],
//! unchanged and already contract-tested. The bridge adds no publication
//! semantics of its own.
//!
//! # Publication Semantics
//!
//! | `stream.publish` | when a publication happens |
//! | --- | --- |
//! | `continuous` | after every bucket, plus once at finish |
//! | `commit` | on every [`DelightBridge::synchronized`], plus once at finish |
//!
//! Per-bucket publishing is safe in `continuous` mode because the ring
//! copies the accumulation into the slot under the accumulation lock (no
//! torn bucket, US2) and drops latest-wins when the client holds everything
//! (the renderer never stalls, R3).
//!
//! [`DelightBridge::synchronized`] is the driver's response to an *applied*
//! `synchronize`. Wiring it to the renderer is the integrator's job: pass a
//! [`StatusCallback`](nsi_ffi_wrap::context::StatusCallback) as
//! `RenderControl`'s `"callback"` argument and call `synchronized()` for the
//! [`RenderStatus::Synchronized`](nsi_ffi_wrap::context::RenderStatus) and
//! [`Restarted`](nsi_ffi_wrap::context::RenderStatus) statuses -- those two
//! are the anchor the scene generation counts.
//!
//! # Threading
//!
//! 3Delight delivers buckets from many threads at once (`spec.md`, Risks).
//! The mitigation is the one named there: the bridge serializes uploads per
//! image, which it gets for free because
//! [`PublicationRing::write_bucket`](crate::ring::PublicationRing::write_bucket)
//! holds the accumulation lock across the whole copy. Everything the write
//! closure touches is `Sync` (atomics, mutexes and the ring), so concurrent
//! invocation from renderer threads is data-race free without any `unsafe`
//! in this module.
//!
//! # Limitations
//!
//! - **Format.** The ndspy driver this bridge rides delivers `f32`, so only
//!   [`LayerFormat::RgbaF32`] is supported. A layer declaring
//!   [`LayerFormat::RgbaF16`] is rejected by [`DelightBridge::new`] with a
//!   typed error; pixel conversion is out of scope.
//! - **Transport.** The bridge is the in-process CPU path
//!   ([`Transport::Callback`]). Uploading buckets straight into GPU-resident
//!   images is a separate task, so the bridge probes only the callback
//!   transport: an explicit `stream.transport "gpu"` or `"shm"` fails loudly
//!   with [`Error::TransportUnavailable`] rather than silently degrading
//!   (R8).
//! - **Errors.** An ndspy callback can only answer with an
//!   [`output::Error`](nsi_ffi_wrap::output::Error) code, which carries no
//!   payload. Every failure is therefore *also* recorded as a typed
//!   [`enum@Error`] and surfaced through [`DelightBridge::error`]; the
//!   integrator must check it after a render.

use crate::{
    Error, Result, StreamClient, StreamConfig, StreamDriver,
    config::PublishMode,
    layer::{Bucket, Extent, Layer, LayerFormat},
    ring::{AcquireToken, Publication},
    transport::{StaticProbe, Transport},
};
use nsi_ffi_wrap::output;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

// ─── DelightBridge ──────────────────────────────────────────────────────────

/// A [`StreamDriver`] fed by 3Delight's display-driver callbacks.
///
/// See the [module documentation](self) for the wiring, the publication
/// semantics and the limitations.
#[derive(Debug)]
pub struct DelightBridge {
    shared: Arc<Shared>,
}

impl DelightBridge {
    /// Open a bridged stream.
    ///
    /// Validates the layer formats, then opens the underlying
    /// [`StreamDriver`] -- which validates the version, the layer set, the
    /// extent and the transport before allocating anything. A failure
    /// leaves nothing to clean up.
    ///
    /// # Errors
    ///
    /// - [`Error::MalformedAttribute`] -- a layer declares
    ///   [`LayerFormat::RgbaF16`], which this bridge cannot deliver (see the
    ///   [module documentation](self), "Limitations").
    /// - [`Error::TransportUnavailable`] -- `stream.transport` explicitly
    ///   asked for a transport the bridge does not implement.
    /// - everything [`StreamDriver::open`] can return.
    pub fn new(
        config: StreamConfig,
        layers: Vec<Layer>,
        extent: Extent,
    ) -> Result<Self> {
        layers.iter().try_for_each(|layer| {
            if LayerFormat::RgbaF32 == layer.format {
                Ok(())
            } else {
                Err(Error::malformed(
                    "outputlayer.scalarformat",
                    format!(
                        "the 3Delight bridge rides the `{}` display driver, \
                         which delivers `float` pixels; `outputlayer` `{}` \
                         declares `{}` and this bridge performs no \
                         conversion",
                        output::FERRIS_F32,
                        layer.name,
                        layer.format
                    ),
                ))
            }
        })?;

        let publish = config.publish;
        let driver = Arc::new(StreamDriver::open(
            config,
            layers.clone(),
            extent,
            &Self::probe(),
        )?);

        Ok(Self {
            shared: Arc::new(Shared {
                driver,
                extent,
                layers: layers.into(),
                publish,
                generation: AtomicU64::new(0),
                open_extent: Mutex::new(None),
                buckets: AtomicU64::new(0),
                finished: AtomicBool::new(false),
                error: Mutex::new(None),
                final_image: Mutex::new(None),
            }),
        })
    }

    /// The probe the bridge negotiates with: the in-process callback path
    /// only.
    ///
    /// Naming a transport the bridge cannot provide is a loud
    /// [`Error::TransportUnavailable`], never a silent downgrade (R8).
    fn probe() -> StaticProbe {
        StaticProbe::all_viable()
            .unviable(
                Transport::GpuShared,
                "the 3Delight bridge uploads buckets through the \
                 in-process CPU path; GPU-resident upload is a separate \
                 task",
            )
            .unviable(
                Transport::Shm,
                "the 3Delight bridge is in-process; use the shared-memory \
                 transport with an out-of-process driver instead",
            )
    }

    // ── Introspection ──────────────────────────────────────────────────────

    /// The driver this bridge feeds.
    #[inline]
    pub fn driver(&self) -> &Arc<StreamDriver> {
        &self.shared.driver
    }

    /// The decoded configuration.
    #[inline]
    pub fn config(&self) -> &StreamConfig {
        self.shared.driver.config()
    }

    /// A client facade on the bridge's ring.
    ///
    /// Identical in shape to every other transport's: a viewer consumes
    /// publications with [`acquire`](StreamClient::acquire) /
    /// [`release`](StreamClient::release) and never learns that a renderer
    /// bucket callback is what filled them (US3).
    pub fn client(&self) -> StreamClient {
        self.shared.driver.client()
    }

    /// The extent the bridge was configured for.
    #[inline]
    pub fn extent(&self) -> Extent {
        self.shared.extent
    }

    /// The extent the renderer reported when it opened the driver, if the
    /// open callback has fired.
    ///
    /// A value differing from [`DelightBridge::extent`] is also recorded in
    /// [`DelightBridge::error`] -- it means the `screen` node and the bridge
    /// disagree.
    pub fn open_extent(&self) -> Option<Extent> {
        *self.shared.open_extent.lock().expect("open-extent mutex")
    }

    /// The current scene generation: the number of applied `synchronize`
    /// calls the bridge has observed.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }

    /// Buckets uploaded into the accumulation buffer so far.
    #[inline]
    pub fn buckets(&self) -> u64 {
        self.shared.buckets.load(Ordering::Acquire)
    }

    /// Whether the finish callback has run (and therefore whether the
    /// stream is closed).
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }

    /// The first failure a callback hit, if any.
    ///
    /// The ndspy callbacks can only answer the renderer with an
    /// [`output::Error`] code, so this is where the typed reason lives.
    /// Check it after every render.
    pub fn error(&self) -> Option<Error> {
        self.shared.error.lock().expect("error mutex").clone()
    }

    /// Take the lease the finish callback latched on the final publication.
    ///
    /// [`StreamDriver::close`] retires the pending publication, so the
    /// bridge acquires the final image *before* closing and parks the lease
    /// here. That keeps the last rendered frame available to a client that
    /// was not polling at the exact moment the render ended, and it is what
    /// holds the stream in `Draining`: the stream reaches `Closed` when this
    /// lease is released, exactly like any other outstanding one.
    ///
    /// Returns `None` when the render published nothing, when the ring was
    /// fully leased at finish time, or when the lease was already taken. A
    /// lease still parked here is released when the bridge is dropped.
    pub fn final_image(&self) -> Option<AcquireToken> {
        self.shared
            .final_image
            .lock()
            .expect("final-image mutex")
            .take()
    }

    // ── Driver Side ────────────────────────────────────────────────────────

    /// Announce that a `synchronize` was applied: bump the scene generation
    /// and publish the accumulation as that generation's state.
    ///
    /// This is the anchor of US2. Call it from the renderer's
    /// `stoppedcallback` for the
    /// [`Synchronized`](nsi_ffi_wrap::context::RenderStatus::Synchronized)
    /// and [`Restarted`](nsi_ffi_wrap::context::RenderStatus::Restarted)
    /// statuses -- see the [module documentation](self).
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] once the finish callback has closed the stream.
    pub fn synchronized(&self) -> Result<Option<Publication>> {
        let generation =
            self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;

        self.shared.driver.commit(generation)
    }

    // ── Callbacks ──────────────────────────────────────────────────────────

    /// The `callback.open` closure.
    ///
    /// Validates the extent and the pixel format the renderer reports
    /// against the bridge's configuration; a mismatch is recorded in
    /// [`DelightBridge::error`] and answered with
    /// [`output::Error::BadParameters`].
    pub fn open_callback(&self) -> output::OpenCallback<'static> {
        let shared = Arc::clone(&self.shared);

        output::OpenCallback::new(
            move |_name: &str,
                  width: usize,
                  height: usize,
                  format: &output::PixelFormat| {
                shared.on_open(width, height, format)
            },
        )
    }

    /// The `callback.write` closure.
    ///
    /// Uploads one bucket into the accumulation buffer, and -- in
    /// [`PublishMode::Continuous`] -- publishes it. Called from many
    /// renderer threads at once; see the [module documentation](self),
    /// "Threading".
    pub fn write_callback(&self) -> output::WriteCallback<'static, f32> {
        let shared = Arc::clone(&self.shared);

        output::WriteCallback::<f32>::new(
            move |_name: &str,
                  width: usize,
                  height: usize,
                  x_min: usize,
                  x_max_plus_one: usize,
                  y_min: usize,
                  y_max_plus_one: usize,
                  format: &output::PixelFormat,
                  bucket: &[f32]| {
                shared.on_write(
                    Reported {
                        width,
                        height,
                        x_min,
                        x_max_plus_one,
                        y_min,
                        y_max_plus_one,
                    },
                    format,
                    bucket,
                )
            },
        )
    }

    /// The `callback.finish` closure.
    ///
    /// Publishes the accumulated image one last time, latches it for
    /// [`DelightBridge::final_image`], then closes the stream so it drains
    /// per the lifecycle.
    pub fn finish_callback(&self) -> output::FinishCallback<'static> {
        let shared = Arc::clone(&self.shared);

        output::FinishCallback::new(
            move |_name: String,
                  _width: usize,
                  _height: usize,
                  _format: output::PixelFormat| shared.on_finish(),
        )
    }
}

impl Drop for DelightBridge {
    fn drop(&mut self) {
        // A latched final image the integrator never took would keep the
        // ring from draining. Hand it back.
        if let Some(token) = self.final_image() {
            self.shared.driver.ring().release(token);
        }
    }
}

// ─── Shared State ───────────────────────────────────────────────────────────

/// The bucket geometry one ndspy write callback reports.
#[derive(Debug, Clone, Copy)]
struct Reported {
    width: usize,
    height: usize,
    x_min: usize,
    x_max_plus_one: usize,
    y_min: usize,
    y_max_plus_one: usize,
}

/// Everything the three closures share with the bridge.
///
/// Every field is `Sync`, which is what makes the write closure safe to call
/// from several renderer threads at once.
#[derive(Debug)]
struct Shared {
    driver: Arc<StreamDriver>,
    extent: Extent,
    layers: Arc<[Layer]>,
    publish: PublishMode,
    generation: AtomicU64,
    open_extent: Mutex<Option<Extent>>,
    buckets: AtomicU64,
    finished: AtomicBool,
    error: Mutex<Option<Error>>,
    final_image: Mutex<Option<AcquireToken>>,
}

impl Shared {
    /// Record `error` (the first one wins) and answer the renderer with
    /// `status`.
    fn fail(&self, error: Error, status: output::Error) -> output::Error {
        let mut recorded = self.error.lock().expect("error mutex");

        if recorded.is_none() {
            *recorded = Some(error);
        }

        status
    }

    /// `Some(status)` when a previous callback already failed: once the
    /// bridge is in a failed state it stops touching the ring.
    fn poisoned(&self) -> Option<output::Error> {
        self.error
            .lock()
            .expect("error mutex")
            .is_some()
            .then_some(output::Error::BadParameters)
    }

    // ── Open ───────────────────────────────────────────────────────────────

    fn on_open(
        &self,
        width: usize,
        height: usize,
        format: &output::PixelFormat,
    ) -> output::Error {
        let reported = Extent::new(width as u32, height as u32);
        *self.open_extent.lock().expect("open-extent mutex") = Some(reported);

        if reported != self.extent {
            return self.fail(
                Error::malformed(
                    "screen.resolution",
                    format!(
                        "the renderer opened the stream at {reported}, the \
                         bridge is configured for {}",
                        self.extent
                    ),
                ),
                output::Error::BadParameters,
            );
        }

        if format.len() != self.layers.len() {
            return self.fail(
                Error::malformed(
                    "outputlayer",
                    format!(
                        "the renderer reports {} layer(s) in the pixel \
                         format, {} `outputlayer`(s) are connected to the \
                         bridge",
                        format.len(),
                        self.layers.len()
                    ),
                ),
                output::Error::BadParameters,
            );
        }

        // Respect what the open callback reports: a layer must occupy
        // exactly as many channels in the flat pixel as it declared, or the
        // bucket copy would silently reinterpret AOVs.
        for (declared, reported) in self.layers.iter().zip(format.iter()) {
            if reported.channels() != declared.channels as usize {
                return self.fail(
                    Error::malformed(
                        "outputlayer.layertype",
                        format!(
                            "the renderer reports {} channel(s) for layer \
                             `{}`, the bridge declared {}",
                            reported.channels(),
                            declared.name,
                            declared.channels
                        ),
                    ),
                    output::Error::BadParameters,
                );
            }
        }

        output::Error::None
    }

    // ── Write ──────────────────────────────────────────────────────────────

    fn on_write(
        &self,
        reported: Reported,
        format: &output::PixelFormat,
        bucket_data: &[f32],
    ) -> output::Error {
        if let Some(status) = self.poisoned() {
            return status;
        }

        let extent = Extent::new(reported.width as u32, reported.height as u32);

        if extent != self.extent {
            return self.fail(
                Error::malformed(
                    "screen.resolution",
                    format!(
                        "a bucket arrived for a {extent} image, the bridge \
                         is configured for {}",
                        self.extent
                    ),
                ),
                output::Error::BadParameters,
            );
        }

        if reported.x_max_plus_one < reported.x_min
            || reported.y_max_plus_one < reported.y_min
        {
            return self.fail(
                Error::invalid_write(format!(
                    "inverted bucket window {}..{} x {}..{}",
                    reported.x_min,
                    reported.x_max_plus_one,
                    reported.y_min,
                    reported.y_max_plus_one
                )),
                output::Error::BadParameters,
            );
        }

        let bucket = Bucket::new(
            reported.x_min as u32,
            reported.y_min as u32,
            (reported.x_max_plus_one - reported.x_min) as u32,
            (reported.y_max_plus_one - reported.y_min) as u32,
        );

        let pixels = bucket.pixels();
        let channels = format.channels();

        if bucket_data.len() < pixels * channels {
            return self.fail(
                Error::invalid_write(format!(
                    "the renderer delivered {} sample(s) for a {}x{} bucket \
                     of {channels} channel(s)",
                    bucket_data.len(),
                    bucket.width,
                    bucket.height
                )),
                output::Error::BadParameters,
            );
        }

        // One repack per bucket per layer: the flat ndspy pixel interleaves
        // every connected `outputlayer`, a publication plane holds exactly
        // one (US4).
        for (index, plane) in format.iter().enumerate() {
            let bytes = repack(
                bucket_data,
                pixels,
                channels,
                plane.offset(),
                plane.channels(),
            );

            if let Err(error) = self.driver.write_bucket(index, bucket, &bytes)
            {
                return self.fail(error, output::Error::Undefined);
            }
        }

        self.buckets.fetch_add(1, Ordering::AcqRel);

        // `continuous`: every bucket is a publication. Safe because the ring
        // copies under the accumulation lock (no torn bucket) and drops
        // latest-wins instead of stalling the renderer.
        if PublishMode::Continuous == self.publish
            && let Err(error) = self.driver.publish_progressive()
        {
            return self.fail(error, output::Error::Undefined);
        }

        output::Error::None
    }

    // ── Finish ─────────────────────────────────────────────────────────────

    fn on_finish(&self) -> output::Error {
        let status = if self.poisoned().is_some() {
            output::Error::BadParameters
        } else {
            let published = match self.publish {
                PublishMode::Commit => {
                    self.driver.commit(self.generation.load(Ordering::Acquire))
                }
                PublishMode::Continuous => self.driver.publish_progressive(),
            };

            match published {
                Ok(_) => {
                    // Close retires the pending publication, so latch it
                    // first; see `DelightBridge::final_image`.
                    if let Some(token) = self.driver.ring().acquire() {
                        *self.final_image.lock().expect("final-image mutex") =
                            Some(token);
                    }

                    output::Error::None
                }
                Err(error) => self.fail(error, output::Error::Undefined),
            }
        };

        self.driver.close();
        self.finished.store(true, Ordering::Release);

        status
    }
}

// ─── Repacking ──────────────────────────────────────────────────────────────

/// Extract one layer's `channels` samples per pixel from the flat,
/// interleaved ndspy bucket and return them as the tightly packed bytes the
/// ring expects.
///
/// The bytes are the native representation of the `f32` samples: the bridge
/// performs no conversion of any kind -- pixel data stays linear and
/// scene-referred (`crate::layer`, colorimetry).
fn repack(
    bucket_data: &[f32],
    pixels: usize,
    stride: usize,
    offset: usize,
    channels: usize,
) -> Vec<u8> {
    (0..pixels)
        .flat_map(|pixel| {
            let base = pixel * stride + offset;

            bucket_data[base..base + channels]
                .iter()
                .flat_map(|sample| sample.to_ne_bytes())
        })
        .collect()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn beauty() -> Vec<Layer> {
        vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32)]
    }

    #[test]
    fn half_float_layers_are_rejected() {
        let error = DelightBridge::new(
            StreamConfig::default(),
            vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16)],
            Extent::new(4, 4),
        )
        .expect_err("the bridge cannot deliver f16");

        assert!(matches!(
            error,
            Error::MalformedAttribute { ref name, .. }
                if name == "outputlayer.scalarformat"
        ));
    }

    #[test]
    fn explicit_gpu_transport_does_not_fall_back() {
        let config = StreamConfig {
            transport: crate::transport::TransportRequest::Explicit(
                Transport::GpuShared,
            ),
            ..StreamConfig::default()
        };

        assert!(matches!(
            DelightBridge::new(config, beauty(), Extent::new(4, 4)),
            Err(Error::TransportUnavailable { .. })
        ));
    }

    #[test]
    fn the_bridge_negotiates_the_callback_transport() {
        let bridge = DelightBridge::new(
            StreamConfig::default(),
            beauty(),
            Extent::new(4, 4),
        )
        .expect("a legal bridge");

        assert_eq!(bridge.driver().transport(), Transport::Callback);
        assert_eq!(bridge.generation(), 0);
        assert_eq!(bridge.open_extent(), None);
        assert_eq!(bridge.error(), None);
    }

    #[test]
    fn repacking_extracts_one_layer() {
        // Two pixels, six channels each: `Ci` rgba at 0, a vector at 4.
        let data = [
            0.0f32, 1.0, 2.0, 3.0, 90.0, 91.0, 10.0, 11.0, 12.0, 13.0, 92.0,
            93.0,
        ];

        let bytes = repack(&data, 2, 6, 0, 4);
        let samples = bytes
            .chunks_exact(4)
            .map(|chunk| {
                f32::from_ne_bytes(chunk.try_into().expect("four bytes"))
            })
            .collect::<Vec<_>>();

        assert_eq!(samples, vec![0.0, 1.0, 2.0, 3.0, 10.0, 11.0, 12.0, 13.0]);
    }
}
