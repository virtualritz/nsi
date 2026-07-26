//! Shared-memory transport -- version-1 frozen layout.
//!
//! The CPU degradation path of US3. The driver creates an anonymous
//! shared-memory segment (`memfd_create`), maps it, and publishes into it;
//! the client receives the file descriptor over the [`stream.channel`
//! rendezvous](crate::channel), maps the same segment and acquires from it.
//! Acquire/release semantics are identical to the in-process
//! [`PublicationRing`](crate::ring::PublicationRing) -- latest-wins, never
//! blocking, driver never writes a leased slot -- but they are driven
//! through the shared header instead of a `Mutex`.
//!
//! # Wire Format (frozen at `stream.version` 1)
//!
//! One header page followed by the ring's pixel planes. **All integers are
//! little-endian and every header field is accessed as an atomic**, so the
//! two ends never need a lock. Offsets are absolute byte offsets into the
//! mapping.
//!
//! ```text
//! 0x0000  magic            u64   b"NSISTRM1" read as little-endian
//! 0x0008  layout_version   u32   1
//! 0x000c  header_bytes     u32   4096
//! 0x0010  width            u32
//! 0x0014  height           u32
//! 0x0018  layer_count      u32   <= 64
//! 0x001c  ring_size        u32   >= 2, <= 32
//! 0x0020  slot_bytes       u64   bytes per slot (sum of all plane sizes)
//! 0x0028  sequence         u64   seqlock: even = stable, odd = mutating
//! 0x0030  scene_generation u64   generation of the latest publication
//! 0x0038  dropped          u64   publications dropped, ring fully leased
//! 0x0040  timeline         u64   monotonic timeline value (R4 equivalent)
//! 0x0048  latest_slot      i64   slot holding the latest publication, -1 none
//! 0x0050  next_serial      u64   next frame serial to hand out (from 1)
//! 0x0058  closed           u32   0 open, 1 closed
//! 0x005c  (reserved)       u32
//! 0x0060  layer table      32 bytes per layer, `layer_count` entries:
//!           +0x00 format        u32   `LayerFormat` wire discriminant
//!           +0x04 channels      u32
//!           +0x08 plane_offset  u64   relative to the slot's first byte
//!           +0x10 plane_bytes   u64
//!           +0x18 (reserved)    u64
//! 0x0860  slot table       48 bytes per slot, `ring_size` entries:
//!           +0x00 state            u32  0 free, 1 writing, 2 published,
//!                                       3 leased
//!           +0x04 (reserved)       u32
//!           +0x08 frame_serial     u64
//!           +0x10 scene_generation u64
//!           +0x18 timeline_value   u64
//!           +0x20 write_stamp      u64  nanoseconds since the Unix epoch
//!           +0x28 width            u32
//!           +0x2c height           u32
//! 0x1000  slot 0 planes, slot 1 planes, ... (each `slot_bytes` long)
//! ```
//!
//! Planes are tightly packed -- no row padding, no plane padding -- in the
//! layer's declared format, linear and scene-referred.
//!
//! # Compatibility And Versioning
//!
//! The layout is frozen for `stream.version` 1. Any change to a field's
//! offset, width or meaning -- including adding a field inside the reserved
//! space in a way older readers would misread -- requires a
//! `stream.version` bump. A client that maps a segment whose `magic` or
//! `layout_version` does not match **must** fail loudly
//! ([`Error::MalformedAttribute`] naming `stream.channel`); it must never
//! guess a layout. Reserved bytes are zero on creation and must be ignored
//! by readers.
//!
//! # Resize
//!
//! A segment has a fixed extent. Resizing allocates a *new* segment whose
//! descriptor is sent in the `Resize` channel message; the client keeps the
//! old mapping alive until its last lease is released.
//!
//! # Failure Modes
//!
//! - `memfd_create`/`ftruncate`/`mmap` failure → [`Error::Io`].
//! - bad magic, unknown layout version, implausible ring or layer count →
//!   [`Error::MalformedAttribute`].
//! - publish after close → [`Error::Closed`].

use crate::{
    Error, Result,
    layer::{Extent, Layer, LayerFormat},
    ring::Publication,
};
use rustix::{
    fs::{MemfdFlags, ftruncate, memfd_create},
    mm::{MapFlags, ProtFlags, mmap, munmap},
};
use std::{
    ffi::c_void,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

// ─── Frozen Layout Constants ────────────────────────────────────────────────

/// `b"NSISTRM1"` read as a little-endian `u64`.
pub const MAGIC: u64 = u64::from_le_bytes(*b"NSISTRM1");

/// Layout version. Bumped only together with `stream.version`.
pub const LAYOUT_VERSION: u32 = 1;

/// Size of the header page.
pub const HEADER_BYTES: usize = 4096;

/// Largest layer count the header page can describe.
pub const MAX_LAYERS: usize = 64;

/// Largest ring size the header page can describe.
pub const MAX_RING: usize = 32;

const OFF_MAGIC: usize = 0x00;
const OFF_LAYOUT_VERSION: usize = 0x08;
const OFF_HEADER_BYTES: usize = 0x0c;
const OFF_WIDTH: usize = 0x10;
const OFF_HEIGHT: usize = 0x14;
const OFF_LAYER_COUNT: usize = 0x18;
const OFF_RING_SIZE: usize = 0x1c;
const OFF_SLOT_BYTES: usize = 0x20;
const OFF_SEQUENCE: usize = 0x28;
const OFF_SCENE_GENERATION: usize = 0x30;
const OFF_DROPPED: usize = 0x38;
const OFF_TIMELINE: usize = 0x40;
const OFF_LATEST_SLOT: usize = 0x48;
const OFF_NEXT_SERIAL: usize = 0x50;
const OFF_CLOSED: usize = 0x58;

const LAYER_TABLE: usize = 0x60;
const LAYER_ENTRY: usize = 0x20;
const LAYER_FORMAT: usize = 0x00;
const LAYER_CHANNELS: usize = 0x04;
const LAYER_PLANE_OFFSET: usize = 0x08;
const LAYER_PLANE_BYTES: usize = 0x10;

const SLOT_TABLE: usize = LAYER_TABLE + MAX_LAYERS * LAYER_ENTRY;
const SLOT_ENTRY: usize = 0x30;
const SLOT_STATE: usize = 0x00;
const SLOT_FRAME_SERIAL: usize = 0x08;
const SLOT_SCENE_GENERATION: usize = 0x10;
const SLOT_TIMELINE_VALUE: usize = 0x18;
const SLOT_WRITE_STAMP: usize = 0x20;
const SLOT_WIDTH: usize = 0x28;
const SLOT_HEIGHT: usize = 0x2c;

/// Slot states, as stored in the slot table.
const STATE_FREE: u32 = 0;
const STATE_WRITING: u32 = 1;
const STATE_PUBLISHED: u32 = 2;
const STATE_LEASED: u32 = 3;

const NO_SLOT: i64 = -1;

// ─── Mapping ────────────────────────────────────────────────────────────────

/// An `mmap`ed region.
#[derive(Debug)]
struct Mapping {
    base: *mut u8,
    bytes: usize,
}

// SAFETY: the mapping is shared memory whose every header field is accessed
// through atomics and whose pixel bytes are handed out only under the slot
// state machine (a slot is either being written by the driver or leased by
// the client, never both). The pointer itself is valid for the lifetime of
// the `Mapping` on every thread.
unsafe impl Send for Mapping {}
// SAFETY: see the `Send` impl.
unsafe impl Sync for Mapping {}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: `base`/`bytes` are exactly what `mmap` returned and the
        // region is unmapped exactly once, here.
        let _ = unsafe { munmap(self.base.cast::<c_void>(), self.bytes) };
    }
}

impl Mapping {
    fn new(fd: BorrowedFd<'_>, bytes: usize) -> Result<Self> {
        // SAFETY: a fresh, anonymous shared mapping of a descriptor sized to
        // at least `bytes`. No existing mapping is replaced (`addr` is
        // null).
        let base = unsafe {
            mmap(
                ptr::null_mut(),
                bytes,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                fd,
                0,
            )
        }
        .map_err(|error| Error::io("mmap of the stream segment", error))?;

        Ok(Self {
            base: base.cast::<u8>(),
            bytes,
        })
    }

    fn u32_at(&self, offset: usize) -> &AtomicU32 {
        debug_assert!(offset + 4 <= self.bytes && offset.is_multiple_of(4));

        // SAFETY: the offset is inside the mapping and 4-byte aligned (the
        // mapping starts page-aligned), and every access to this location
        // goes through this same atomic type.
        unsafe { &*self.base.add(offset).cast::<AtomicU32>() }
    }

    fn u64_at(&self, offset: usize) -> &AtomicU64 {
        debug_assert!(offset + 8 <= self.bytes && offset.is_multiple_of(8));

        // SAFETY: see `u32_at`; the offset is 8-byte aligned.
        unsafe { &*self.base.add(offset).cast::<AtomicU64>() }
    }

    fn i64_at(&self, offset: usize) -> &AtomicI64 {
        debug_assert!(offset + 8 <= self.bytes && offset.is_multiple_of(8));

        // SAFETY: see `u32_at`; the offset is 8-byte aligned.
        unsafe { &*self.base.add(offset).cast::<AtomicI64>() }
    }

    fn bytes_at(&self, offset: usize, len: usize) -> &[u8] {
        debug_assert!(offset + len <= self.bytes);

        // SAFETY: the range is inside the mapping. The caller only calls
        // this for a slot it holds a lease on, which the driver never
        // writes.
        unsafe { std::slice::from_raw_parts(self.base.add(offset), len) }
    }

    #[allow(clippy::mut_from_ref)]
    fn bytes_at_mut(&self, offset: usize, len: usize) -> &mut [u8] {
        debug_assert!(offset + len <= self.bytes);

        // SAFETY: the range is inside the mapping. The caller only calls
        // this through a `ShmWriteGuard`, which exists at most once per slot
        // and only for a slot in `STATE_WRITING` -- no client may read it
        // and no second writer may exist.
        unsafe { std::slice::from_raw_parts_mut(self.base.add(offset), len) }
    }
}

// ─── Segment ────────────────────────────────────────────────────────────────

/// A mapped stream segment: the descriptor, the mapping and the decoded
/// header.
#[derive(Debug)]
pub struct ShmSegment {
    fd: OwnedFd,
    map: Mapping,
    layers: Vec<Layer>,
    plane_offsets: Vec<usize>,
    extent: Extent,
    ring: usize,
    slot_bytes: usize,
}

impl ShmSegment {
    /// Create and map a new segment.
    ///
    /// # Errors
    ///
    /// [`Error::MalformedAttribute`] for a layer or ring count the frozen
    /// header cannot describe, [`Error::Io`] for a failing syscall.
    pub fn create(
        layers: &[Layer],
        extent: Extent,
        ring: usize,
    ) -> Result<Self> {
        if layers.is_empty() || layers.len() > MAX_LAYERS {
            Err(Error::malformed(
                "stream.channel",
                format!(
                    "the version-1 shm layout describes 1..={MAX_LAYERS} \
                     layers, got {}",
                    layers.len()
                ),
            ))?;
        }
        if !(2..=MAX_RING).contains(&ring) {
            Err(Error::malformed(
                "stream.ring",
                format!(
                    "the version-1 shm layout describes 2..={MAX_RING} \
                     slots, got {ring}"
                ),
            ))?;
        }
        if extent.is_empty() {
            Err(Error::malformed(
                "stream.channel",
                format!("degenerate extent {extent}"),
            ))?;
        }

        let (plane_offsets, slot_bytes) = plane_layout(layers, extent);
        let bytes = HEADER_BYTES + ring * slot_bytes;

        let fd = memfd_create("nsi-stream", MemfdFlags::CLOEXEC)
            .map_err(|error| Error::io("memfd_create", error))?;
        ftruncate(&fd, bytes as u64).map_err(|error| {
            Error::io("ftruncate of the stream segment", error)
        })?;

        let map = Mapping::new(fd.as_fd(), bytes)?;

        // The header is written once, before the descriptor is shared, so
        // no seqlock protection is needed here.
        map.u64_at(OFF_MAGIC).store(MAGIC, Ordering::Relaxed);
        map.u32_at(OFF_LAYOUT_VERSION)
            .store(LAYOUT_VERSION, Ordering::Relaxed);
        map.u32_at(OFF_HEADER_BYTES)
            .store(HEADER_BYTES as u32, Ordering::Relaxed);
        map.u32_at(OFF_WIDTH).store(extent.width, Ordering::Relaxed);
        map.u32_at(OFF_HEIGHT)
            .store(extent.height, Ordering::Relaxed);
        map.u32_at(OFF_LAYER_COUNT)
            .store(layers.len() as u32, Ordering::Relaxed);
        map.u32_at(OFF_RING_SIZE)
            .store(ring as u32, Ordering::Relaxed);
        map.u64_at(OFF_SLOT_BYTES)
            .store(slot_bytes as u64, Ordering::Relaxed);
        map.i64_at(OFF_LATEST_SLOT)
            .store(NO_SLOT, Ordering::Relaxed);
        // Serials start at 1 so that timeline value 0 means "nothing
        // published yet", as in the in-process ring.
        map.u64_at(OFF_NEXT_SERIAL).store(1, Ordering::Relaxed);

        layers.iter().zip(&plane_offsets).enumerate().for_each(
            |(index, (layer, offset))| {
                let entry = LAYER_TABLE + index * LAYER_ENTRY;
                map.u32_at(entry + LAYER_FORMAT)
                    .store(layer.format.as_wire(), Ordering::Relaxed);
                map.u32_at(entry + LAYER_CHANNELS)
                    .store(layer.channels, Ordering::Relaxed);
                map.u64_at(entry + LAYER_PLANE_OFFSET)
                    .store(*offset as u64, Ordering::Relaxed);
                map.u64_at(entry + LAYER_PLANE_BYTES)
                    .store(layer.plane_bytes(extent) as u64, Ordering::Relaxed);
            },
        );

        // Publish the header to any thread that maps the segment later.
        map.u64_at(OFF_SEQUENCE).store(2, Ordering::Release);

        Ok(Self {
            fd,
            map,
            layers: layers.to_vec(),
            plane_offsets,
            extent,
            ring,
            slot_bytes,
        })
    }

    /// Map an existing segment received over the rendezvous channel.
    ///
    /// Validates magic and layout version loudly -- a mismatch is never
    /// guessed around.
    ///
    /// # Errors
    ///
    /// [`Error::MalformedAttribute`] when the header is not a version-1
    /// stream segment, [`Error::Io`] when the mapping fails.
    pub fn attach(fd: OwnedFd) -> Result<Self> {
        // Map the header page first, then remap once the real size is
        // known.
        let probe = Mapping::new(fd.as_fd(), HEADER_BYTES)?;

        if MAGIC != probe.u64_at(OFF_MAGIC).load(Ordering::Acquire) {
            Err(Error::malformed(
                "stream.channel",
                "the mapped segment does not start with the `NSISTRM1` magic",
            ))?;
        }

        let version = probe.u32_at(OFF_LAYOUT_VERSION).load(Ordering::Acquire);
        if LAYOUT_VERSION != version {
            Err(Error::malformed(
                "stream.channel",
                format!(
                    "shm layout version {version} is not supported \
                     (supported: {LAYOUT_VERSION})"
                ),
            ))?;
        }

        let layer_count =
            probe.u32_at(OFF_LAYER_COUNT).load(Ordering::Acquire) as usize;
        let ring = probe.u32_at(OFF_RING_SIZE).load(Ordering::Acquire) as usize;
        let slot_bytes =
            probe.u64_at(OFF_SLOT_BYTES).load(Ordering::Acquire) as usize;
        let extent = Extent::new(
            probe.u32_at(OFF_WIDTH).load(Ordering::Acquire),
            probe.u32_at(OFF_HEIGHT).load(Ordering::Acquire),
        );

        if layer_count == 0
            || layer_count > MAX_LAYERS
            || !(2..=MAX_RING).contains(&ring)
            || slot_bytes == 0
            || extent.is_empty()
        {
            Err(Error::malformed(
                "stream.channel",
                format!(
                    "implausible header: {layer_count} layers, {ring} slots, \
                     {slot_bytes} bytes per slot, extent {extent}"
                ),
            ))?;
        }

        let layers = (0..layer_count)
            .map(|index| {
                let entry = LAYER_TABLE + index * LAYER_ENTRY;
                let wire =
                    probe.u32_at(entry + LAYER_FORMAT).load(Ordering::Acquire);

                LayerFormat::from_wire(wire)
                    .map(|format| {
                        Layer::new(
                            format!("layer{index}"),
                            format!("layer{index}"),
                            format,
                            probe
                                .u32_at(entry + LAYER_CHANNELS)
                                .load(Ordering::Acquire),
                        )
                    })
                    .ok_or_else(|| {
                        Error::malformed(
                            "stream.channel",
                            format!(
                                "unknown pixel format {wire} in the layer table"
                            ),
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        let plane_offsets = (0..layer_count)
            .map(|index| {
                probe
                    .u64_at(
                        LAYER_TABLE + index * LAYER_ENTRY + LAYER_PLANE_OFFSET,
                    )
                    .load(Ordering::Acquire) as usize
            })
            .collect();

        drop(probe);

        let map = Mapping::new(fd.as_fd(), HEADER_BYTES + ring * slot_bytes)?;

        Ok(Self {
            fd,
            map,
            layers,
            plane_offsets,
            extent,
            ring,
            slot_bytes,
        })
    }

    /// The segment's descriptor, for passing over the rendezvous channel.
    #[inline]
    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// The layers described by the header.
    ///
    /// A segment carries formats and sizes only; layer *names* travel in the
    /// channel's `Open` message, so an attached segment reports placeholder
    /// names.
    #[inline]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The segment's extent. Fixed for the segment's lifetime.
    #[inline]
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Number of ring slots.
    #[inline]
    pub const fn ring_size(&self) -> usize {
        self.ring
    }

    /// Publications dropped because every slot was leased.
    #[inline]
    pub fn dropped(&self) -> u64 {
        self.map.u64_at(OFF_DROPPED).load(Ordering::Acquire)
    }

    /// The current timeline value.
    #[inline]
    pub fn timeline(&self) -> u64 {
        self.map.u64_at(OFF_TIMELINE).load(Ordering::Acquire)
    }

    /// Whether the driver closed the stream.
    #[inline]
    pub fn is_closed(&self) -> bool {
        0 != self.map.u32_at(OFF_CLOSED).load(Ordering::Acquire)
    }

    fn slot_state(&self, slot: usize) -> &AtomicU32 {
        self.map.u32_at(SLOT_TABLE + slot * SLOT_ENTRY + SLOT_STATE)
    }

    fn slot_u64(&self, slot: usize, field: usize) -> &AtomicU64 {
        self.map.u64_at(SLOT_TABLE + slot * SLOT_ENTRY + field)
    }

    fn slot_u32(&self, slot: usize, field: usize) -> &AtomicU32 {
        self.map.u32_at(SLOT_TABLE + slot * SLOT_ENTRY + field)
    }

    fn plane_range(&self, slot: usize, layer: usize) -> Option<(usize, usize)> {
        self.layers.get(layer).map(|description| {
            (
                HEADER_BYTES
                    + slot * self.slot_bytes
                    + self.plane_offsets[layer],
                description.plane_bytes(self.extent),
            )
        })
    }

    fn publication(&self, slot: usize) -> Publication {
        Publication {
            image_index: slot,
            frame_serial: self
                .slot_u64(slot, SLOT_FRAME_SERIAL)
                .load(Ordering::Acquire),
            scene_generation: self
                .slot_u64(slot, SLOT_SCENE_GENERATION)
                .load(Ordering::Acquire),
            timeline_value: self
                .slot_u64(slot, SLOT_TIMELINE_VALUE)
                .load(Ordering::Acquire),
            extent: Extent::new(
                self.slot_u32(slot, SLOT_WIDTH).load(Ordering::Acquire),
                self.slot_u32(slot, SLOT_HEIGHT).load(Ordering::Acquire),
            ),
        }
    }
}

/// Plane offsets inside one slot, plus the slot's total size.
fn plane_layout(layers: &[Layer], extent: Extent) -> (Vec<usize>, usize) {
    let mut cursor = 0;
    let offsets = layers
        .iter()
        .map(|layer| {
            let offset = cursor;
            cursor += layer.plane_bytes(extent);
            offset
        })
        .collect();

    (offsets, cursor)
}

// ─── Driver End ─────────────────────────────────────────────────────────────

/// The driver end of a shared-memory stream.
#[derive(Debug, Clone)]
pub struct ShmDriver {
    segment: Arc<ShmSegment>,
}

impl ShmDriver {
    /// Create the segment.
    ///
    /// # Errors
    ///
    /// See [`ShmSegment::create`].
    pub fn create(
        layers: &[Layer],
        extent: Extent,
        ring: usize,
    ) -> Result<Self> {
        Ok(Self {
            segment: Arc::new(ShmSegment::create(layers, extent, ring)?),
        })
    }

    /// The mapped segment.
    #[inline]
    pub fn segment(&self) -> &Arc<ShmSegment> {
        &self.segment
    }

    /// Claim a slot for writing, or `Ok(None)` when every slot is leased.
    ///
    /// Never blocks. A slot holding an unacquired publication is recycled
    /// (latest-wins); the drop counter increments only when the ring is
    /// fully leased.
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`ShmDriver::close`].
    pub fn begin_write(&self) -> Result<Option<ShmWriteGuard>> {
        if self.segment.is_closed() {
            Err(Error::Closed)?;
        }

        let claim = |from: u32| {
            (0..self.segment.ring).find(|slot| {
                self.segment
                    .slot_state(*slot)
                    .compare_exchange(
                        from,
                        STATE_WRITING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
            })
        };

        let Some(slot) = claim(STATE_FREE).or_else(|| {
            claim(STATE_PUBLISHED).inspect(|slot| {
                let _ =
                    self.segment.map.i64_at(OFF_LATEST_SLOT).compare_exchange(
                        *slot as i64,
                        NO_SLOT,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    );
            })
        }) else {
            self.segment
                .map
                .u64_at(OFF_DROPPED)
                .fetch_add(1, Ordering::AcqRel);

            return Ok(None);
        };

        Ok(Some(ShmWriteGuard {
            segment: Arc::clone(&self.segment),
            slot,
            published: false,
        }))
    }

    /// Announce the slot held by `guard` as the latest publication.
    ///
    /// # Errors
    ///
    /// [`Error::Closed`] after [`ShmDriver::close`].
    pub fn publish(
        &self,
        mut guard: ShmWriteGuard,
        scene_generation: u64,
    ) -> Result<Publication> {
        if self.segment.is_closed() {
            Err(Error::Closed)?;
        }

        let slot = guard.slot;
        let segment = &self.segment;
        let serial = segment
            .map
            .u64_at(OFF_NEXT_SERIAL)
            .fetch_add(1, Ordering::AcqRel);

        segment
            .slot_u64(slot, SLOT_FRAME_SERIAL)
            .store(serial, Ordering::Release);
        segment
            .slot_u64(slot, SLOT_SCENE_GENERATION)
            .store(scene_generation, Ordering::Release);
        segment
            .slot_u64(slot, SLOT_TIMELINE_VALUE)
            .store(serial, Ordering::Release);
        segment
            .slot_u64(slot, SLOT_WRITE_STAMP)
            .store(now_nanos(), Ordering::Release);
        segment
            .slot_u32(slot, SLOT_WIDTH)
            .store(segment.extent.width, Ordering::Release);
        segment
            .slot_u32(slot, SLOT_HEIGHT)
            .store(segment.extent.height, Ordering::Release);

        // Seqlock around the header-wide mutation: readers that sample the
        // header see either the whole previous publication or the whole new
        // one.
        let sequence = segment.map.u64_at(OFF_SEQUENCE);
        sequence.fetch_add(1, Ordering::AcqRel);

        let previous = segment
            .map
            .i64_at(OFF_LATEST_SLOT)
            .swap(slot as i64, Ordering::AcqRel);
        segment
            .map
            .u64_at(OFF_SCENE_GENERATION)
            .store(scene_generation, Ordering::Release);
        segment
            .map
            .u64_at(OFF_TIMELINE)
            .store(serial, Ordering::Release);

        sequence.fetch_add(1, Ordering::AcqRel);

        segment
            .slot_state(slot)
            .store(STATE_PUBLISHED, Ordering::Release);
        guard.published = true;

        // Latest-wins: the superseded, unacquired publication returns to the
        // free list.
        if NO_SLOT != previous && previous != slot as i64 {
            let _ = segment.slot_state(previous as usize).compare_exchange(
                STATE_PUBLISHED,
                STATE_FREE,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }

        Ok(segment.publication(slot))
    }

    /// Publications dropped because every slot was leased.
    #[inline]
    pub fn dropped(&self) -> u64 {
        self.segment.dropped()
    }

    /// Mark the segment closed and return the final timeline value.
    pub fn close(&self) -> u64 {
        let final_value = self
            .segment
            .map
            .u64_at(OFF_NEXT_SERIAL)
            .load(Ordering::Acquire);

        self.segment
            .map
            .u64_at(OFF_TIMELINE)
            .store(final_value, Ordering::Release);
        self.segment
            .map
            .i64_at(OFF_LATEST_SLOT)
            .store(NO_SLOT, Ordering::Release);
        self.segment
            .map
            .u32_at(OFF_CLOSED)
            .store(1, Ordering::Release);

        final_value
    }
}

/// Exclusive write access to one shared-memory slot.
#[derive(Debug)]
pub struct ShmWriteGuard {
    segment: Arc<ShmSegment>,
    slot: usize,
    published: bool,
}

impl ShmWriteGuard {
    /// Slot index being written.
    #[inline]
    pub const fn image_index(&self) -> usize {
        self.slot
    }

    /// Mutable access to one layer's plane in the slot.
    pub fn plane_mut(&mut self, layer: usize) -> Option<&mut [u8]> {
        self.segment
            .plane_range(self.slot, layer)
            .map(|(offset, len)| self.segment.map.bytes_at_mut(offset, len))
    }

    /// Fill every plane with `byte`.
    pub fn fill(&mut self, byte: u8) {
        (0..self.segment.layers.len()).for_each(|layer| {
            if let Some(plane) = self.plane_mut(layer) {
                plane.fill(byte);
            }
        });
    }
}

impl Drop for ShmWriteGuard {
    fn drop(&mut self) {
        if !self.published {
            let _ = self.segment.slot_state(self.slot).compare_exchange(
                STATE_WRITING,
                STATE_FREE,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
}

// ─── Client End ─────────────────────────────────────────────────────────────

/// The client end of a shared-memory stream.
#[derive(Debug, Clone)]
pub struct ShmClient {
    segment: Arc<ShmSegment>,
}

impl ShmClient {
    /// Map a segment received over the rendezvous channel.
    ///
    /// # Errors
    ///
    /// See [`ShmSegment::attach`].
    pub fn attach(fd: OwnedFd) -> Result<Self> {
        Ok(Self {
            segment: Arc::new(ShmSegment::attach(fd)?),
        })
    }

    /// Build a client on an already mapped segment (in-process fixtures and
    /// the parity tests).
    pub fn new(segment: Arc<ShmSegment>) -> Self {
        Self { segment }
    }

    /// The mapped segment.
    #[inline]
    pub fn segment(&self) -> &Arc<ShmSegment> {
        &self.segment
    }

    /// Take a lease on the latest publication.
    ///
    /// Never blocks; `None` means "nothing new".
    pub fn acquire(&self) -> Option<ShmAcquireToken> {
        let slot = self
            .segment
            .map
            .i64_at(OFF_LATEST_SLOT)
            .swap(NO_SLOT, Ordering::AcqRel);

        if NO_SLOT == slot {
            None
        } else {
            let slot = slot as usize;

            self.segment
                .slot_state(slot)
                .compare_exchange(
                    STATE_PUBLISHED,
                    STATE_LEASED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .ok()
                .map(|_| ShmAcquireToken {
                    segment: Arc::clone(&self.segment),
                    publication: self.segment.publication(slot),
                })
        }
    }

    /// Return a lease.
    pub fn release(&self, token: ShmAcquireToken) {
        let slot = token.publication.image_index;
        drop(token);

        let _ = self.segment.slot_state(slot).compare_exchange(
            STATE_LEASED,
            STATE_FREE,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    /// Publications dropped because every slot was leased.
    #[inline]
    pub fn dropped(&self) -> u64 {
        self.segment.dropped()
    }

    /// Whether the driver closed the stream and no lease is outstanding.
    pub fn is_drained(&self) -> bool {
        self.segment.is_closed()
            && !(0..self.segment.ring).any(|slot| {
                STATE_LEASED
                    == self.segment.slot_state(slot).load(Ordering::Acquire)
            })
    }
}

/// A client lease on one shared-memory publication.
///
/// Deliberately not `Clone`: a lease is unique.
#[derive(Debug)]
pub struct ShmAcquireToken {
    segment: Arc<ShmSegment>,
    publication: Publication,
}

impl ShmAcquireToken {
    /// The publication this lease describes.
    #[inline]
    pub const fn publication(&self) -> &Publication {
        &self.publication
    }

    /// Read-only access to one layer's plane.
    pub fn plane(&self, layer: usize) -> Option<&[u8]> {
        self.segment
            .plane_range(self.publication.image_index, layer)
            .map(|(offset, len)| self.segment.map.bytes_at(offset, len))
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_nanos() as u64)
        .unwrap_or_default()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_tables_fit_the_header_page() {
        const { assert!(SLOT_TABLE + MAX_RING * SLOT_ENTRY <= HEADER_BYTES) };
        assert_eq!(SLOT_TABLE, 0x860);
    }

    #[test]
    fn attach_rejects_a_foreign_segment() {
        let fd = memfd_create("not-a-stream", MemfdFlags::CLOEXEC)
            .expect("memfd_create");
        ftruncate(&fd, HEADER_BYTES as u64).expect("ftruncate");

        assert!(matches!(
            ShmSegment::attach(fd),
            Err(Error::MalformedAttribute { .. })
        ));
    }
}
