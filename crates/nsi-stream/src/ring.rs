//! The publication ring -- driver-owned images, client leases.
//!
//! # Model
//!
//! The driver owns `N >= 2` publication slots (R3) plus one **accumulation
//! buffer** per layer. Renderer threads deliver buckets into the
//! accumulation buffer; a publication *copies* the accumulation into a free
//! slot and announces it. Clients acquire the latest publication as a lease
//! and release it when done.
//!
//! ```text
//! renderer threads --write_bucket--> accumulation --copy--> slot --acquire--> client
//! ```
//!
//! # Invariants (`contracts/publication-lifecycle.md`)
//!
//! - **The driver never writes a slot a client holds.** A leased slot is
//!   excluded from [`PublicationRing::begin_write`] entirely.
//! - **Publication never blocks renderer progress.** If every slot is
//!   leased, [`PublicationRing::begin_write`] returns `Ok(None)`
//!   immediately, the publication is dropped (latest-wins) and
//!   [`PublicationRing::dropped`] increments.
//! - **Frame serials are strictly monotonic**, and so is the timeline value
//!   -- in this CPU ring they are the same number, which is what
//!   [`Publication::timeline_value`] carries. Both start at 1, so the
//!   timeline's initial value 0 means "nothing published yet", exactly like
//!   a freshly created timeline semaphore.
//! - **Scene generation** is the count of applied `synchronize` calls
//!   observed by the driver, and every publication carries exactly one.
//! - **No torn bucket.** The accumulation lock is held for the whole of a
//!   bucket write and for the whole of the accumulation → slot copy, so a
//!   published slot can never contain a half-written bucket.
//!
//! # Concurrency
//!
//! Two locks, always taken in this order: accumulation, then ring state.
//! Bucket writes take the accumulation lock only; acquire/release/
//! begin_write take the ring-state lock only; a publication takes both.
//!
//! Slot storage is `Arc`-backed, which is what makes resize safe without
//! `unsafe`: a lease holds its own `Arc` to the buffer it was handed, so
//! [`PublicationRing::resize`] can swap fresh buffers into every slot while
//! outstanding leases keep reading the old ones. The pre-resize buffer is
//! freed when the last lease on it is released, and is never written again.

use crate::{
    Error, Result,
    config::PublishMode,
    layer::{Bucket, Extent, Layer},
    timeline::CpuTimeline,
};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};

// ─── Publication ────────────────────────────────────────────────────────────

/// A publication announcement.
///
/// This is what the driver sends over the rendezvous channel (or hands to
/// the in-process callback) and what a client needs in order to sample the
/// image: which slot, which serial, which scene state, and which timeline
/// value to wait on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Publication {
    /// Ring slot index the pixels live in.
    pub image_index: usize,
    /// Strictly monotonic publication counter, starting at 1. Zero means
    /// "nothing published yet".
    pub frame_serial: u64,
    /// Number of applied `synchronize` calls this image was rendered from.
    pub scene_generation: u64,
    /// Timeline value to wait on before sampling.
    pub timeline_value: u64,
    /// Extent of this image. May differ from the ring's current extent
    /// after a resize.
    pub extent: Extent,
}

// ─── Slot Storage ───────────────────────────────────────────────────────────

/// One slot's pixel planes -- one plane per layer, tightly packed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlotStorage {
    extent: Extent,
    planes: Vec<Vec<u8>>,
}

impl SlotStorage {
    fn new(layers: &[Layer], extent: Extent) -> Self {
        Self {
            extent,
            planes: layers
                .iter()
                .map(|layer| vec![0u8; layer.plane_bytes(extent)])
                .collect(),
        }
    }

    /// Extent these planes were allocated for.
    #[inline]
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Read-only access to one layer's plane.
    #[inline]
    pub fn plane(&self, layer: usize) -> Option<&[u8]> {
        self.planes.get(layer).map(Vec::as_slice)
    }

    /// Total bytes held by all planes.
    #[inline]
    pub fn bytes(&self) -> usize {
        self.planes.iter().map(Vec::len).sum()
    }
}

// ─── Slot ───────────────────────────────────────────────────────────────────

/// Lifecycle of one ring slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SlotState {
    /// Reusable by the driver.
    Free,
    /// A [`WriteGuard`] holds the storage.
    Writing,
    /// Holds the latest publication, not yet acquired.
    Published,
    /// A client holds a lease. Off-limits to the driver.
    Leased,
}

#[derive(Debug)]
struct Slot {
    state: SlotState,
    /// `None` exactly while a [`WriteGuard`] holds the storage.
    storage: Option<Arc<SlotStorage>>,
    publication: Option<Publication>,
}

// ─── Ring State ─────────────────────────────────────────────────────────────

#[derive(Debug)]
struct RingState {
    slots: Vec<Slot>,
    extent: Extent,
    /// Bumped by every resize; stamped into leases and write guards so
    /// pre-resize storage is never mistaken for current storage.
    epoch: u64,
    next_serial: u64,
    latest: Option<usize>,
    closed: bool,
}

#[derive(Debug)]
struct Accumulation {
    extent: Extent,
    planes: Vec<Vec<u8>>,
}

// ─── Guards And Tokens ──────────────────────────────────────────────────────

/// Exclusive write access to one free ring slot.
///
/// Dropping the guard without publishing returns the slot to the ring
/// untouched by the client -- an abandoned write is not a publication.
#[derive(Debug)]
pub struct WriteGuard<'a> {
    ring: &'a PublicationRing,
    slot: usize,
    epoch: u64,
    /// `None` only after [`PublicationRing::publish`] took the storage.
    storage: Option<Arc<SlotStorage>>,
}

impl WriteGuard<'_> {
    /// Slot index being written.
    #[inline]
    pub const fn image_index(&self) -> usize {
        self.slot
    }

    /// Extent of the slot's planes.
    #[inline]
    pub fn extent(&self) -> Extent {
        self.storage
            .as_ref()
            .map(|storage| storage.extent)
            .unwrap_or_default()
    }

    /// Mutable access to one layer's plane.
    pub fn plane_mut(&mut self, layer: usize) -> Option<&mut [u8]> {
        self.storage
            .as_mut()
            // The ring moved its own `Arc` out of the slot, and a slot in
            // `Writing` state can not be leased, so this is the only strong
            // reference and `get_mut` always succeeds.
            .and_then(Arc::get_mut)
            .and_then(|storage| storage.planes.get_mut(layer))
            .map(Vec::as_mut_slice)
    }

    /// Fill every plane with `byte`. Handy for fixtures that tag an image
    /// with a generation.
    pub fn fill(&mut self, byte: u8) {
        (0..self.ring.layers.len()).for_each(|layer| {
            if let Some(plane) = self.plane_mut(layer) {
                plane.fill(byte);
            }
        });
    }
}

impl Drop for WriteGuard<'_> {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.take() {
            self.ring.abandon(self.slot, self.epoch, storage);
        }
    }
}

/// A client lease on one publication.
///
/// The token keeps its own reference to the pixel storage, so it stays
/// readable across a [`PublicationRing::resize`] and even across
/// [`PublicationRing::close`], until it is handed back to
/// [`PublicationRing::release`].
///
/// Deliberately not `Clone`: a lease is unique, and releasing one twice
/// would hand the driver a slot a client still reads.
#[derive(Debug)]
pub struct AcquireToken {
    publication: Publication,
    epoch: u64,
    storage: Arc<SlotStorage>,
}

impl AcquireToken {
    /// The publication this lease describes.
    #[inline]
    pub const fn publication(&self) -> &Publication {
        &self.publication
    }

    /// Extent of the leased image.
    #[inline]
    pub const fn extent(&self) -> Extent {
        self.publication.extent
    }

    /// Timeline value to wait on before sampling.
    #[inline]
    pub const fn timeline_value(&self) -> u64 {
        self.publication.timeline_value
    }

    /// Read-only access to one layer's plane.
    #[inline]
    pub fn plane(&self, layer: usize) -> Option<&[u8]> {
        self.storage.plane(layer)
    }

    /// Total bytes of the leased image.
    #[inline]
    pub fn bytes(&self) -> usize {
        self.storage.bytes()
    }
}

// ─── PublicationRing ────────────────────────────────────────────────────────

/// The driver-owned ring of publication images.
#[derive(Debug)]
pub struct PublicationRing {
    layers: Arc<[Layer]>,
    publish_mode: PublishMode,
    state: Mutex<RingState>,
    accumulation: Mutex<Accumulation>,
    timeline: CpuTimeline,
    generation: AtomicU64,
    dropped: AtomicU64,
    published: AtomicU64,
}

impl PublicationRing {
    /// Allocate a ring.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidWrite`] when the ring size is below
    /// [`StreamConfig::MIN_RING`](crate::StreamConfig::MIN_RING), when no
    /// layer is connected, or when the extent is degenerate. Validation
    /// happens before any allocation, so a failed construction leaves
    /// nothing to clean up.
    pub fn new(
        layers: Vec<Layer>,
        extent: Extent,
        ring_size: usize,
        publish_mode: PublishMode,
    ) -> Result<Self> {
        if ring_size < 2 {
            Err(Error::invalid_write(format!(
                "ring size must be at least 2, got {ring_size}"
            )))?;
        }
        if layers.is_empty() {
            Err(Error::invalid_write(
                "at least one `outputlayer` must be connected",
            ))?;
        }
        if extent.is_empty() {
            Err(Error::invalid_write(format!("degenerate extent {extent}")))?;
        }

        let slots = (0..ring_size)
            .map(|_| Slot {
                state: SlotState::Free,
                storage: Some(Arc::new(SlotStorage::new(&layers, extent))),
                publication: None,
            })
            .collect();

        let accumulation = Accumulation {
            extent,
            planes: layers
                .iter()
                .map(|layer| vec![0u8; layer.plane_bytes(extent)])
                .collect(),
        };

        Ok(Self {
            layers: layers.into(),
            publish_mode,
            state: Mutex::new(RingState {
                slots,
                extent,
                epoch: 0,
                next_serial: 1,
                latest: None,
                closed: false,
            }),
            accumulation: Mutex::new(accumulation),
            timeline: CpuTimeline::new(),
            generation: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            published: AtomicU64::new(0),
        })
    }

    // ── Introspection ──────────────────────────────────────────────────────

    /// The connected layers, in publication plane order.
    #[inline]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The configured publish mode.
    #[inline]
    pub const fn publish_mode(&self) -> PublishMode {
        self.publish_mode
    }

    /// The timeline every publication signals.
    #[inline]
    pub const fn timeline(&self) -> &CpuTimeline {
        &self.timeline
    }

    /// Number of slots.
    pub fn ring_size(&self) -> usize {
        self.locked().slots.len()
    }

    /// The extent the next publication will use.
    pub fn extent(&self) -> Extent {
        self.locked().extent
    }

    /// The scene generation the driver is currently rendering.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Publications dropped because every slot was leased (latest-wins).
    ///
    /// A publication that is superseded before the client acquired it is
    /// *not* counted here -- that is ordinary latest-wins replacement, not
    /// back pressure.
    #[inline]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Acquire)
    }

    /// Publications announced so far.
    #[inline]
    pub fn published(&self) -> u64 {
        self.published.load(Ordering::Acquire)
    }

    /// Whether a publication is waiting to be acquired.
    pub fn has_publication(&self) -> bool {
        self.locked().latest.is_some()
    }

    /// Whether the ring is closed.
    pub fn is_closed(&self) -> bool {
        self.locked().closed
    }

    /// Whether the ring is closed *and* every lease was returned
    /// (`Draining` → `Closed`).
    pub fn is_drained(&self) -> bool {
        let state = self.locked();

        state.closed
            && !state
                .slots
                .iter()
                .any(|slot| SlotState::Leased == slot.state)
    }

    // ── Driver Side ────────────────────────────────────────────────────────

    /// Copy a rendered bucket into the accumulation buffer.
    ///
    /// `data` is tightly packed, `bucket.width * layer.bytes_per_pixel()`
    /// bytes per row, top row first. The accumulation lock is held for the
    /// whole copy, which is what makes a published image free of torn
    /// buckets.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidWrite`] when the layer index is unknown, the bucket
    /// does not fit the current extent, or `data` has the wrong length.
    /// [`Error::Closed`] after [`PublicationRing::close`].
    pub fn write_bucket(
        &self,
        layer: usize,
        bucket: Bucket,
        data: &[u8],
    ) -> Result<()> {
        self.with_bucket(layer, bucket, data.len(), |row, target| {
            let start = row * target.len();
            target.copy_from_slice(&data[start..start + target.len()]);
        })
    }

    /// Fill a bucket region with a single byte value.
    ///
    /// Fixtures use this to tag a bucket with a generation or a pass
    /// number; renderers use [`PublicationRing::write_bucket`].
    pub fn fill_bucket(
        &self,
        layer: usize,
        bucket: Bucket,
        byte: u8,
    ) -> Result<()> {
        let expected = self
            .layers
            .get(layer)
            .map(|layer| bucket.pixels() * layer.bytes_per_pixel())
            .unwrap_or_default();

        self.with_bucket(layer, bucket, expected, |_row, target| {
            target.fill(byte)
        })
    }

    /// Claim a free slot for writing.
    ///
    /// Returns `Ok(None)` -- immediately, never blocking -- when every slot
    /// is leased by a client. The drop counter increments in that case.
    ///
    /// A slot holding an unacquired publication is recycled: the publication
    /// it held is superseded (latest-wins). Leased slots are never touched.
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`PublicationRing::close`].
    pub fn begin_write(&self) -> Result<Option<WriteGuard<'_>>> {
        let mut state = self.locked();

        if state.closed {
            Err(Error::Closed)?;
        }

        let free = state
            .slots
            .iter()
            .position(|slot| SlotState::Free == slot.state);

        // Latest-wins: an unacquired publication may be recycled so the
        // renderer never stalls.
        let Some(index) = free.or_else(|| {
            state
                .slots
                .iter()
                .position(|slot| SlotState::Published == slot.state)
        }) else {
            self.dropped.fetch_add(1, Ordering::AcqRel);
            return Ok(None);
        };

        if free.is_none() {
            state.latest = None;
        }

        let epoch = state.epoch;
        let slot = &mut state.slots[index];
        slot.state = SlotState::Writing;
        slot.publication = None;

        Ok(slot.storage.take().map(|storage| WriteGuard {
            ring: self,
            slot: index,
            epoch,
            storage: Some(storage),
        }))
    }

    /// Announce the slot held by `guard` as the latest publication.
    ///
    /// Assigns the next frame serial, stamps the scene generation, signals
    /// the timeline with the publication's value and returns the
    /// announcement.
    ///
    /// Returns `Ok(None)` when a [`PublicationRing::resize`] invalidated the
    /// guard while it was being written: the stale pixels are discarded and
    /// the drop counter increments.
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`PublicationRing::close`].
    pub fn publish(
        &self,
        mut guard: WriteGuard<'_>,
        scene_generation: u64,
    ) -> Result<Option<Publication>> {
        let index = guard.slot;
        let epoch = guard.epoch;
        let storage = guard.storage.take().expect("guard holds its storage");

        let publication = {
            let mut state = self.locked();

            if state.closed {
                Err(Error::Closed)?;
            }

            if epoch != state.epoch {
                // The slot was re-provisioned by a resize while this guard
                // was writing. Drop the stale pixels; the slot itself is
                // already back in service.
                self.dropped.fetch_add(1, Ordering::AcqRel);
                return Ok(None);
            }

            let serial = state.next_serial;
            state.next_serial += 1;

            let publication = Publication {
                image_index: index,
                frame_serial: serial,
                scene_generation,
                timeline_value: serial,
                extent: storage.extent,
            };

            // Latest-wins: the previous, unacquired publication is
            // superseded and its slot returns to the free list.
            if let Some(previous) = state.latest.take()
                && SlotState::Published == state.slots[previous].state
            {
                state.slots[previous].state = SlotState::Free;
                state.slots[previous].publication = None;
            }

            let slot = &mut state.slots[index];
            slot.state = SlotState::Published;
            slot.storage = Some(storage);
            slot.publication = Some(publication);
            state.latest = Some(index);

            publication
        };

        self.published.fetch_add(1, Ordering::AcqRel);
        self.timeline.signal(publication.timeline_value);

        Ok(Some(publication))
    }

    /// Publish the accumulation as the state of scene generation
    /// `generation` -- the driver's response to an applied `synchronize`.
    ///
    /// This is the atomic hand-off of US2: the accumulation lock is held
    /// across the copy, so the published image contains samples from exactly
    /// one generation and no torn bucket.
    ///
    /// Returns `Ok(None)` when the publication had to be dropped
    /// (ring fully leased, or a concurrent resize).
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`PublicationRing::close`].
    pub fn commit(&self, generation: u64) -> Result<Option<Publication>> {
        self.generation.store(generation, Ordering::Release);
        self.snapshot(generation)
    }

    /// Publish the current accumulation at the current scene generation.
    ///
    /// In [`PublishMode::Commit`] this is a documented no-op returning
    /// `Ok(None)`: progressive refinement must not be visible between
    /// commits. In [`PublishMode::Continuous`] it publishes, tagged with the
    /// generation of the last [`PublicationRing::commit`].
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`PublicationRing::close`].
    pub fn publish_progressive(&self) -> Result<Option<Publication>> {
        if PublishMode::Commit == self.publish_mode {
            if self.is_closed() {
                Err(Error::Closed)?;
            }

            Ok(None)
        } else {
            self.snapshot(self.generation())
        }
    }

    /// Reallocate the ring at `extent`.
    ///
    /// Every slot receives freshly allocated storage. Outstanding leases
    /// keep the buffers they were handed and stay valid until released; the
    /// pre-resize buffers are freed on release and never written again. The
    /// pending publication, if any, is discarded -- the next publication
    /// uses the new extent.
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`PublicationRing::close`],
    /// [`Error::InvalidWrite`] for a degenerate extent.
    pub fn resize(&self, extent: Extent) -> Result<()> {
        if extent.is_empty() {
            Err(Error::invalid_write(format!("degenerate extent {extent}")))?;
        }

        let mut accumulation =
            self.accumulation.lock().expect("accumulation mutex");
        let mut state = self.locked();

        if state.closed {
            Err(Error::Closed)?;
        }

        accumulation.extent = extent;
        accumulation.planes = self
            .layers
            .iter()
            .map(|layer| vec![0u8; layer.plane_bytes(extent)])
            .collect();

        state.epoch += 1;
        state.extent = extent;
        state.latest = None;

        state.slots.iter_mut().for_each(|slot| {
            slot.storage =
                Some(Arc::new(SlotStorage::new(&self.layers, extent)));
            slot.publication = None;
            slot.state = match slot.state {
                // A lease keeps its own `Arc` to the old buffer; the slot
                // stays off-limits until it comes back.
                SlotState::Leased => SlotState::Leased,
                // A guard writing an old-extent image is invalidated; its
                // slot is already usable again.
                _ => SlotState::Free,
            };
        });

        Ok(())
    }

    /// Stop publishing and signal the final timeline value.
    ///
    /// Returns that final value, which is one past the last publication's.
    /// Outstanding leases stay valid and drain on release; watch
    /// [`PublicationRing::is_drained`] for the `Draining` → `Closed`
    /// transition.
    pub fn close(&self) -> u64 {
        let final_value = {
            let mut state = self.locked();
            state.closed = true;
            state.latest = None;
            state.next_serial
        };

        self.timeline.signal(final_value);

        final_value
    }

    // ── Client Side ────────────────────────────────────────────────────────

    /// Take a lease on the latest publication, if there is a new one.
    ///
    /// Never blocks and never waits on renderer progress: returns `None`
    /// when nothing was published since the last acquire.
    pub fn acquire(&self) -> Option<AcquireToken> {
        let mut state = self.locked();
        let index = state.latest.take()?;
        let epoch = state.epoch;
        let slot = &mut state.slots[index];

        if SlotState::Published == slot.state {
            slot.state = SlotState::Leased;

            slot.publication.zip(slot.storage.clone()).map(
                |(publication, storage)| AcquireToken {
                    publication,
                    epoch,
                    storage,
                },
            )
        } else {
            None
        }
    }

    /// Return a lease. The slot becomes reusable by the driver.
    pub fn release(&self, token: AcquireToken) {
        let index = token.publication.image_index;
        let epoch = token.epoch;
        // Drop the client's reference first so the slot's own `Arc` becomes
        // unique again and the buffer can be written in place.
        drop(token.storage);

        let mut state = self.locked();
        let current_epoch = state.epoch;

        if let Some(slot) = state.slots.get_mut(index)
            && SlotState::Leased == slot.state
        {
            slot.state = SlotState::Free;
            slot.publication = None;

            // A pre-resize lease has nothing to hand back: `resize` already
            // re-provisioned its slot with fresh storage.
            debug_assert!(
                epoch == current_epoch || slot.storage.is_some(),
                "a re-provisioned slot always carries fresh storage"
            );
        }
    }

    // ── Internals ──────────────────────────────────────────────────────────

    fn locked(&self) -> MutexGuard<'_, RingState> {
        self.state.lock().expect("ring state mutex")
    }

    /// Return an abandoned (never published) slot to the ring.
    fn abandon(&self, index: usize, epoch: u64, storage: Arc<SlotStorage>) {
        let mut state = self.locked();

        if epoch == state.epoch
            && let Some(slot) = state.slots.get_mut(index)
            && SlotState::Writing == slot.state
        {
            slot.state = SlotState::Free;
            slot.storage = Some(storage);
        }
    }

    /// Copy the accumulation into a free slot and publish it.
    fn snapshot(&self, generation: u64) -> Result<Option<Publication>> {
        // Taking the accumulation lock first is what "after in-flight
        // buckets complete" means: no bucket write can be half-applied
        // while the copy runs.
        let accumulation =
            self.accumulation.lock().expect("accumulation mutex");

        let Some(mut guard) = self.begin_write()? else {
            return Ok(None);
        };

        accumulation
            .planes
            .iter()
            .enumerate()
            .for_each(|(layer, plane)| {
                if let Some(target) = guard.plane_mut(layer)
                    && target.len() == plane.len()
                {
                    target.copy_from_slice(plane);
                }
            });

        drop(accumulation);

        self.publish(guard, generation)
    }

    /// Shared bucket-write plumbing: validate, then run `write` per row.
    fn with_bucket(
        &self,
        layer: usize,
        bucket: Bucket,
        data_len: usize,
        mut write: impl FnMut(usize, &mut [u8]),
    ) -> Result<()> {
        let description = self.layers.get(layer).ok_or_else(|| {
            Error::invalid_write(format!(
                "layer {layer} is not connected ({} layers)",
                self.layers.len()
            ))
        })?;

        if self.is_closed() {
            Err(Error::Closed)?;
        }

        let bytes_per_pixel = description.bytes_per_pixel();
        let bucket_row_bytes = bucket.width as usize * bytes_per_pixel;

        if data_len != bucket.pixels() * bytes_per_pixel {
            Err(Error::invalid_write(format!(
                "expected {} bytes for a {}x{} bucket of `{}`, got {data_len}",
                bucket.pixels() * bytes_per_pixel,
                bucket.width,
                bucket.height,
                description.name
            )))?;
        }

        let mut accumulation =
            self.accumulation.lock().expect("accumulation mutex");
        let extent = accumulation.extent;

        if !bucket.fits(extent) {
            Err(Error::invalid_write(format!(
                "bucket {}x{}+{}+{} does not fit extent {extent}",
                bucket.width, bucket.height, bucket.x, bucket.y
            )))?;
        }

        let row_bytes = description.row_bytes(extent);
        let plane = accumulation
            .planes
            .get_mut(layer)
            .expect("one accumulation plane per layer");

        (0..bucket.height as usize).for_each(|row| {
            let start = (bucket.y as usize + row) * row_bytes
                + bucket.x as usize * bytes_per_pixel;

            write(row, &mut plane[start..start + bucket_row_bytes]);
        });

        Ok(())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::LayerFormat;

    fn ring(size: usize, mode: PublishMode) -> PublicationRing {
        PublicationRing::new(
            vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32)],
            Extent::new(4, 4),
            size,
            mode,
        )
        .expect("a legal ring")
    }

    #[test]
    fn ring_below_minimum_is_rejected() {
        assert!(
            PublicationRing::new(
                vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32)],
                Extent::new(4, 4),
                1,
                PublishMode::Commit,
            )
            .is_err()
        );
    }

    #[test]
    fn abandoned_write_is_not_a_publication() {
        let ring = ring(2, PublishMode::Commit);

        drop(ring.begin_write().expect("open ring"));

        assert!(!ring.has_publication());
        assert_eq!(ring.published(), 0);
        assert!(ring.acquire().is_none());
    }

    #[test]
    fn serials_are_strictly_monotonic() {
        let ring = ring(3, PublishMode::Commit);

        let serials = (0..4)
            .map(|generation| {
                ring.commit(generation)
                    .expect("open ring")
                    .expect("a free slot")
                    .frame_serial
            })
            .collect::<Vec<_>>();

        assert_eq!(serials, vec![1, 2, 3, 4]);
    }

    #[test]
    fn commit_mode_ignores_progressive_publications() {
        let ring = ring(3, PublishMode::Commit);

        assert!(ring.publish_progressive().expect("open ring").is_none());
        assert_eq!(ring.published(), 0);
    }
}
