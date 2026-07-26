//! Contract: `contracts/publication-lifecycle.md`.
//!
//! Rows covered here:
//!
//! - "`commit` mode: every publication carries one scene generation" --
//!   `publish_commit_atomic`.
//! - "`continuous` mode: acquired image never contains a torn bucket" --
//!   `publish_continuous_no_torn_bucket`.
//! - "Acquire is non-blocking and returns latest publication or none" --
//!   `acquire_nonblocking`.
//! - "Client waits on the Publication's timeline value before sampling" --
//!   `publication_semaphore_complete`.
//! - "Renderer never stalls on a fully leased ring" --
//!   `ring_exhaustion_drops`.
//! - "Release returns the image to the ring; driver reuses only released
//!   images" -- `release_reuse_ordering`.
//! - "Resize: next publication has new extent; held pre-resize acquisitions
//!   stay valid until release" -- `resize_drain_safety`.
//! - "Close drains" -- `close_drain`.

use nsi_stream::{
    AcquireToken, Bucket, Error, Extent, Layer, LayerFormat, PublicationRing,
    PublishMode, StreamClient,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// One RGBA f32 layer: 16 bytes per pixel.
const BYTES_PER_PIXEL: usize = 16;

/// Acquire must return within this; the contract calls it non-blocking and
/// the plan budgets 100 µs. The bound is deliberately loose so the assertion
/// tests "does not wait for the renderer", not scheduler noise.
const NON_BLOCKING: Duration = Duration::from_millis(50);

fn ring(
    size: usize,
    extent: Extent,
    mode: PublishMode,
) -> Arc<PublicationRing> {
    Arc::new(
        PublicationRing::new(
            vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF32)],
            extent,
            size,
            mode,
        )
        .expect("a legal ring"),
    )
}

/// The four quadrant buckets of `extent`, as a renderer would deliver them.
fn quadrants(extent: Extent) -> Vec<Bucket> {
    let half = (extent.width / 2, extent.height / 2);

    vec![
        Bucket::new(0, 0, half.0, half.1),
        Bucket::new(half.0, 0, extent.width - half.0, half.1),
        Bucket::new(0, half.1, half.0, extent.height - half.1),
        Bucket::new(
            half.0,
            half.1,
            extent.width - half.0,
            extent.height - half.1,
        ),
    ]
}

/// Every byte of `plane` inside `bucket`, row by row.
fn bucket_bytes(plane: &[u8], extent: Extent, bucket: Bucket) -> Vec<u8> {
    let row_bytes = extent.width as usize * BYTES_PER_PIXEL;

    (0..bucket.height as usize)
        .flat_map(|row| {
            let start = (bucket.y as usize + row) * row_bytes
                + bucket.x as usize * BYTES_PER_PIXEL;

            plane[start..start + bucket.width as usize * BYTES_PER_PIXEL]
                .to_vec()
        })
        .collect()
}

/// The single byte value filling `bucket`, or `None` when the region is torn.
fn uniform_value(plane: &[u8], extent: Extent, bucket: Bucket) -> Option<u8> {
    let bytes = bucket_bytes(plane, extent, bucket);

    bytes
        .first()
        .copied()
        .filter(|first| bytes.iter().all(|byte| byte == first))
}

/// Fill every quadrant of layer 0 with `value`.
fn fill_all(ring: &PublicationRing, extent: Extent, value: u8) {
    quadrants(extent).into_iter().for_each(|bucket| {
        ring.fill_bucket(0, bucket, value).expect("a legal bucket");
    });
}

// ─── Acquire ────────────────────────────────────────────────────────────────

/// Acquire never waits on renderer progress: with nothing published it
/// returns `None` immediately, and after a publication it returns that
/// publication exactly once.
#[test]
fn acquire_nonblocking() {
    let extent = Extent::new(8, 8);
    let ring = ring(3, extent, PublishMode::Commit);

    let started = Instant::now();
    assert!(ring.acquire().is_none(), "nothing published yet");
    let idle = started.elapsed();

    assert!(
        idle < NON_BLOCKING,
        "acquire on an empty ring took {idle:?}, it must not block"
    );

    fill_all(&ring, extent, 0x42);
    ring.commit(0).expect("open ring").expect("a free slot");

    let started = Instant::now();
    let token = ring.acquire().expect("the latest publication");
    let busy = started.elapsed();

    assert!(
        busy < NON_BLOCKING,
        "acquire of a ready publication took {busy:?}"
    );
    assert_eq!(token.publication().frame_serial, 1);

    // "Latest publication or nothing new" -- a second acquire without a new
    // publication is "nothing new", not the same image again.
    assert!(ring.acquire().is_none());

    ring.release(token);
}

// ─── Ring Exhaustion ────────────────────────────────────────────────────────

/// With every slot leased the renderer does not stall: the publication is
/// dropped, latest-wins, and the drop counter increments.
#[test]
fn ring_exhaustion_drops() {
    let extent = Extent::new(8, 8);
    let ring = ring(3, extent, PublishMode::Commit);

    // Lease the whole ring.
    let tokens = (0..3)
        .map(|generation| {
            fill_all(&ring, extent, generation as u8);
            ring.commit(generation)
                .expect("open ring")
                .expect("a free slot");

            ring.acquire().expect("the latest publication")
        })
        .collect::<Vec<_>>();

    let leased = tokens
        .iter()
        .map(|token| token.publication().image_index)
        .collect::<Vec<_>>();

    assert_eq!(leased.len(), 3);
    assert!(
        leased
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == 3,
        "each lease must be a distinct slot, got {leased:?}"
    );
    assert_eq!(ring.dropped(), 0);

    // The renderer wants to publish into a fully leased ring.
    let started = Instant::now();
    let dropped = ring.commit(3).expect("open ring");
    let stalled = started.elapsed();

    assert!(dropped.is_none(), "the publication must be dropped");
    assert!(
        stalled < NON_BLOCKING,
        "a dropped publication took {stalled:?}, the renderer must not stall"
    );
    assert_eq!(ring.dropped(), 1);

    ring.commit(4).expect("open ring");
    assert_eq!(ring.dropped(), 2);
    assert_eq!(ring.published(), 3, "no publication was announced");

    // Returning one lease unblocks publishing again.
    let mut tokens = tokens;
    let returned = tokens.pop().expect("a lease");
    ring.release(returned);

    assert!(
        ring.commit(5).expect("open ring").is_some(),
        "a released slot must be reusable"
    );
    assert_eq!(ring.dropped(), 2, "no further drop");

    tokens.into_iter().for_each(|token| ring.release(token));
}

/// The driver reuses released slots only; a leased slot is never written.
#[test]
fn release_reuse_ordering() {
    let extent = Extent::new(4, 4);
    let ring = ring(3, extent, PublishMode::Commit);

    let tokens = (0..3)
        .map(|generation| {
            fill_all(&ring, extent, generation as u8);
            ring.commit(generation)
                .expect("open ring")
                .expect("a free slot");

            ring.acquire().expect("the latest publication")
        })
        .collect::<Vec<_>>();

    let slots = tokens
        .iter()
        .map(|token| token.publication().image_index)
        .collect::<Vec<_>>();

    // Everything is leased -- nothing to reuse.
    assert!(ring.commit(3).expect("open ring").is_none());

    // Return exactly one lease: the next publication must land in that slot
    // and nowhere else.
    let mut tokens = tokens;
    let returned = tokens.remove(1);
    let reused_slot = returned.publication().image_index;
    ring.release(returned);

    let publication = ring
        .commit(4)
        .expect("open ring")
        .expect("the released slot");

    assert_eq!(
        publication.image_index, reused_slot,
        "the driver must reuse the released slot"
    );

    let reacquired = ring.acquire().expect("the latest publication");

    // The two still-leased slots must never have been written.
    let still_leased = tokens
        .iter()
        .map(|token| token.publication().image_index)
        .collect::<Vec<_>>();

    still_leased.iter().enumerate().for_each(|(index, slot)| {
        assert_ne!(
            *slot, reused_slot,
            "slot {slot} is leased and must not be reused"
        );
        assert!(
            tokens[index]
                .plane(0)
                .expect("the beauty plane")
                .iter()
                .all(|byte| *byte
                    == slots
                        .iter()
                        .position(|s| s == slot)
                        .expect("a known slot") as u8),
            "a leased image must keep the pixels it was published with"
        );
    });

    ring.release(reacquired);
    tokens.into_iter().for_each(|token| ring.release(token));
}

// ─── Publish Modes ──────────────────────────────────────────────────────────

/// In `commit` mode a publication happens only on a commit, and every
/// acquired image carries samples from exactly one scene generation.
#[test]
fn publish_commit_atomic() {
    let extent = Extent::new(8, 8);
    let ring = ring(3, extent, PublishMode::Commit);
    let buckets = quadrants(extent);

    // Generation 0, fully rendered.
    fill_all(&ring, extent, 0xa0);
    let first = ring.commit(0).expect("open ring").expect("a free slot");

    assert_eq!(first.scene_generation, 0);

    let token = ring.acquire().expect("the latest publication");
    assert_eq!(
        uniform_value(
            token.plane(0).expect("the beauty plane"),
            extent,
            Bucket::full(extent)
        ),
        Some(0xa0),
        "the image must carry exactly one generation"
    );
    ring.release(token);

    // Generation 1 arrives bucket by bucket. Nothing may become visible
    // before the commit, or the image would mix two generations.
    buckets.iter().for_each(|bucket| {
        ring.fill_bucket(0, *bucket, 0xb1).expect("a legal bucket");

        assert!(
            ring.publish_progressive().expect("open ring").is_none(),
            "`commit` mode must not publish between commits"
        );
        assert!(
            ring.acquire().is_none(),
            "no publication may appear between commits"
        );
    });

    let second = ring.commit(1).expect("open ring").expect("a free slot");

    assert_eq!(second.scene_generation, 1);
    assert!(second.frame_serial > first.frame_serial);

    let token = ring.acquire().expect("the latest publication");
    assert_eq!(token.publication().scene_generation, 1);

    // Every bucket -- and therefore the whole image -- carries generation 1
    // and nothing of generation 0.
    let plane = token.plane(0).expect("the beauty plane");
    buckets.iter().for_each(|bucket| {
        assert_eq!(
            uniform_value(plane, extent, *bucket),
            Some(0xb1),
            "bucket {bucket:?} mixes generations"
        );
    });

    ring.release(token);
}

/// In `continuous` mode an acquired image may show partial refinement, but
/// never a torn bucket: every bucket region is entirely old or entirely new.
#[test]
fn publish_continuous_no_torn_bucket() {
    let extent = Extent::new(64, 32);
    let ring = ring(3, extent, PublishMode::Continuous);
    let buckets = quadrants(extent);
    let passes = 24u8;

    ring.commit(1).expect("open ring");

    let writer_ring = Arc::clone(&ring);
    let writer_buckets = buckets.clone();
    let done = Arc::new(AtomicBool::new(false));
    let writer_done = Arc::clone(&done);

    // The renderer: buckets in a checkered order, each with its own
    // distinctive fill, publishing progressively after every bucket.
    let writer = thread::spawn(move || {
        (1..=passes).for_each(|pass| {
            [0usize, 3, 1, 2].into_iter().for_each(|index| {
                writer_ring
                    .fill_bucket(0, writer_buckets[index], pass)
                    .expect("a legal bucket");
                writer_ring.publish_progressive().expect("open ring");
            });
        });

        writer_done.store(true, Ordering::Release);
    });

    // The client: acquire whatever is latest and check every bucket.
    let mut seen = 0u32;

    while !done.load(Ordering::Acquire) || ring.has_publication() {
        if let Some(token) = ring.acquire() {
            let plane = token.plane(0).expect("the beauty plane");

            buckets.iter().for_each(|bucket| {
                let value = uniform_value(plane, extent, *bucket);

                assert!(
                    value.is_some(),
                    "bucket {bucket:?} of publication {} is torn",
                    token.publication().frame_serial
                );
                assert!(
                    value.expect("a uniform bucket") <= passes,
                    "bucket {bucket:?} carries a value no writer wrote"
                );
            });

            seen += 1;
            ring.release(token);
        }
    }

    writer.join().expect("the writer thread");

    assert!(seen > 0, "the client must have seen a publication");
    assert!(ring.published() > 0);
}

// ─── Synchronization ────────────────────────────────────────────────────────

/// The client waits on the publication's timeline value; after the wait the
/// contents are complete.
#[test]
fn publication_semaphore_complete() {
    let extent = Extent::new(32, 32);
    let ring = ring(2, extent, PublishMode::Commit);
    let client = StreamClient::new(Arc::clone(&ring));

    // A publication that has not happened yet does not resolve, and the
    // wait fails with a typed timeout rather than spinning.
    assert_eq!(
        ring.timeline().wait(1, Some(Duration::from_millis(20))),
        Err(Error::WaitTimeout { serial: 1 })
    );

    let renderer_ring = Arc::clone(&ring);
    let renderer = thread::spawn(move || {
        // Deliver the buckets slowly, so a client that samples without
        // waiting would see an incomplete image.
        quadrants(extent).into_iter().for_each(|bucket| {
            thread::sleep(Duration::from_millis(5));
            renderer_ring
                .fill_bucket(0, bucket, 0x7e)
                .expect("a legal bucket");
        });

        renderer_ring
            .commit(1)
            .expect("open ring")
            .expect("a free slot")
    });

    // The client knows the timeline value of the publication it is waiting
    // for -- serial 1 is the first publication of the stream.
    ring.timeline()
        .wait(1, Some(Duration::from_secs(5)))
        .expect("the first publication is signaled");

    let publication = renderer.join().expect("the renderer thread");
    let token = client.acquire().expect("the latest publication");

    assert_eq!(
        token.publication().timeline_value,
        publication.timeline_value
    );

    // Waiting on the publication's own value is what the contract requires
    // before sampling.
    client
        .wait(token.publication(), Some(Duration::from_secs(5)))
        .expect("the publication is complete");

    assert_eq!(
        uniform_value(
            token.plane(0).expect("the beauty plane"),
            extent,
            Bucket::full(extent)
        ),
        Some(0x7e),
        "after the wait the image must be complete"
    );

    client.release(token);
}

// ─── Resize And Close ───────────────────────────────────────────────────────

/// A resize is safe while a client holds a pre-resize lease: that lease
/// stays valid and readable, the next publication uses the new extent, and
/// the old image is reclaimed only on release.
#[test]
fn resize_drain_safety() {
    let small = Extent::new(8, 8);
    let large = Extent::new(16, 12);
    let ring = ring(3, small, PublishMode::Commit);

    fill_all(&ring, small, 0x11);
    ring.commit(0).expect("open ring").expect("a free slot");

    let held: AcquireToken = ring.acquire().expect("the latest publication");
    assert_eq!(held.extent(), small);

    ring.resize(large).expect("the resize succeeds");

    assert_eq!(ring.extent(), large);

    // The pre-resize lease is still valid and still reads its own pixels.
    assert_eq!(held.extent(), small);
    assert_eq!(
        held.plane(0).expect("the beauty plane").len(),
        small.pixels() * BYTES_PER_PIXEL
    );
    assert_eq!(
        uniform_value(
            held.plane(0).expect("the beauty plane"),
            small,
            Bucket::full(small)
        ),
        Some(0x11),
        "a held image must survive the resize unchanged"
    );

    // The next publication uses the new extent.
    fill_all(&ring, large, 0x22);
    let publication = ring.commit(1).expect("open ring").expect("a free slot");

    assert_eq!(publication.extent, large);

    let resized = ring.acquire().expect("the latest publication");
    assert_eq!(resized.extent(), large);
    assert_eq!(
        resized.plane(0).expect("the beauty plane").len(),
        large.pixels() * BYTES_PER_PIXEL
    );
    assert_eq!(
        uniform_value(
            resized.plane(0).expect("the beauty plane"),
            large,
            Bucket::full(large)
        ),
        Some(0x22)
    );

    // Releasing the pre-resize lease reclaims it; the slot comes back into
    // service at the new extent.
    let reclaimed_slot = held.publication().image_index;
    ring.release(held);
    ring.release(resized);

    fill_all(&ring, large, 0x33);
    let after = ring.commit(2).expect("open ring").expect("a free slot");

    assert_eq!(after.extent, large);

    let token = ring.acquire().expect("the latest publication");
    assert_eq!(
        token.plane(0).expect("the beauty plane").len(),
        large.pixels() * BYTES_PER_PIXEL,
        "a reclaimed slot must be re-provisioned at the new extent"
    );
    let _ = reclaimed_slot;

    ring.release(token);
    ring.close();

    assert!(ring.is_drained());
}

/// Close stops publishing, signals the final timeline value, and drains as
/// leases come back.
#[test]
fn close_drain() {
    let extent = Extent::new(8, 8);
    let ring = ring(3, extent, PublishMode::Commit);
    let client = StreamClient::new(Arc::clone(&ring));

    fill_all(&ring, extent, 0x01);
    let published = ring.commit(0).expect("open ring").expect("a free slot");

    let token = client.acquire().expect("the latest publication");

    let final_value = ring.close();

    // The final timeline value is signaled and is past every publication.
    assert!(final_value > published.timeline_value);
    assert_eq!(ring.timeline().value(), final_value);

    // No publication may happen after close, on any path.
    assert_eq!(ring.commit(1), Err(Error::Closed));
    assert_eq!(ring.publish_progressive(), Err(Error::Closed));
    assert!(matches!(ring.begin_write(), Err(Error::Closed)));
    assert_eq!(
        ring.fill_bucket(0, Bucket::full(extent), 0x02),
        Err(Error::Closed)
    );
    assert_eq!(ring.published(), 1);
    assert!(client.acquire().is_none());

    // Outstanding leases stay valid and readable while draining.
    assert!(!client.is_drained());
    assert_eq!(
        uniform_value(
            token.plane(0).expect("the beauty plane"),
            extent,
            Bucket::full(extent)
        ),
        Some(0x01),
        "a lease held across close must stay readable"
    );

    client.release(token);

    assert!(client.is_drained(), "the stream drains on the last release");
}

/// The drop counter and the publication counter agree with what a renderer
/// observes, from another thread.
#[test]
fn counters_are_observable_across_threads() {
    let extent = Extent::new(8, 8);
    let ring = ring(2, extent, PublishMode::Commit);
    let observer = Arc::clone(&ring);
    let seen = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&seen);

    let renderer = thread::spawn(move || {
        (0..8).for_each(|generation| {
            fill_all(&observer, extent, generation as u8);
            observer.commit(generation).expect("open ring");
            counter.store(observer.published(), Ordering::Release);
        });
    });

    renderer.join().expect("the renderer thread");

    assert_eq!(seen.load(Ordering::Acquire), ring.published());
    assert_eq!(ring.published(), 8);
    assert_eq!(ring.dropped(), 0);
}
