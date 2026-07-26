//! Contract: `contracts/publication-lifecycle.md`, CPU-transport rows.
//!
//! - "CPU/shm transport preserves all rows above with generation counter
//!   semantics" -- `transport_shm_parity`.
//! - "Client loss: driver detects channel close and honors
//!   `stream.onclientloss`" -- `client_loss_behavior`.
#![cfg(feature = "shm")]

use nsi_stream::{
    AcquireToken, ClientLoss, Error, Extent, Layer, LayerFormat,
    PublicationRing, PublishMode, Transport,
    channel::{ClientChannel, DriverChannel, Message, OpenMessage},
    transport::shm::{ShmAcquireToken, ShmClient, ShmDriver},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

const RING: usize = 3;
const EXTENT: Extent = Extent {
    width: 8,
    height: 4,
};

fn layers() -> Vec<Layer> {
    vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16)]
}

// ─── Parity ─────────────────────────────────────────────────────────────────

/// One observable step of the publication scenario.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// The slot a publication landed in, or `None` when it was dropped.
    Published(Option<usize>),
    /// Frame serial, scene generation and the pixel tag of an acquired
    /// image; `None` for "nothing new".
    Acquired(Option<(u64, u64, u8)>),
    /// The drop counter.
    Dropped(u64),
}

/// The publication surface both transports must implement identically.
trait Backend {
    /// Write a whole image tagged with `value` and publish it.
    fn publish(&mut self, generation: u64, value: u8) -> Option<usize>;
    /// Acquire the latest publication and keep the lease.
    fn acquire_and_hold(&mut self) -> Option<(u64, u64, u8)>;
    /// Return the oldest outstanding lease.
    fn release_one(&mut self);
    /// Publications dropped because every slot was leased.
    fn dropped(&self) -> u64;
}

struct CpuBackend {
    ring: PublicationRing,
    held: Vec<AcquireToken>,
}

impl Backend for CpuBackend {
    fn publish(&mut self, generation: u64, value: u8) -> Option<usize> {
        self.ring
            .begin_write()
            .expect("open ring")
            .and_then(|mut guard| {
                guard.fill(value);

                self.ring
                    .publish(guard, generation)
                    .expect("open ring")
                    .map(|publication| publication.image_index)
            })
    }

    fn acquire_and_hold(&mut self) -> Option<(u64, u64, u8)> {
        self.ring.acquire().map(|token| {
            let observed = (
                token.publication().frame_serial,
                token.publication().scene_generation,
                token.plane(0).expect("the beauty plane")[0],
            );
            self.held.push(token);

            observed
        })
    }

    fn release_one(&mut self) {
        if !self.held.is_empty() {
            self.ring.release(self.held.remove(0));
        }
    }

    fn dropped(&self) -> u64 {
        self.ring.dropped()
    }
}

struct ShmBackend {
    driver: ShmDriver,
    client: ShmClient,
    held: Vec<ShmAcquireToken>,
}

impl Backend for ShmBackend {
    fn publish(&mut self, generation: u64, value: u8) -> Option<usize> {
        self.driver
            .begin_write()
            .expect("open segment")
            .map(|mut guard| {
                guard.fill(value);

                self.driver
                    .publish(guard, generation)
                    .expect("open segment")
                    .image_index
            })
    }

    fn acquire_and_hold(&mut self) -> Option<(u64, u64, u8)> {
        self.client.acquire().map(|token| {
            let observed = (
                token.publication().frame_serial,
                token.publication().scene_generation,
                token.plane(0).expect("the beauty plane")[0],
            );
            self.held.push(token);

            observed
        })
    }

    fn release_one(&mut self) {
        if !self.held.is_empty() {
            self.client.release(self.held.remove(0));
        }
    }

    fn dropped(&self) -> u64 {
        self.driver.dropped()
    }
}

/// The scenario every transport must reproduce step for step: latest-wins,
/// "nothing new" on a second acquire, no stall on a fully leased ring, and a
/// drop counter that only counts back pressure.
fn scenario(backend: &mut dyn Backend) -> Vec<Step> {
    // Nothing published yet, then one publication acquired exactly once.
    let mut steps = vec![
        Step::Acquired(backend.acquire_and_hold()),
        Step::Published(backend.publish(0, 0xa0)),
        Step::Acquired(backend.acquire_and_hold()),
        Step::Acquired(backend.acquire_and_hold()),
    ];

    backend.release_one();

    // Latest-wins: the unacquired publication is superseded, not queued.
    steps.push(Step::Published(backend.publish(1, 0xb0)));
    steps.push(Step::Published(backend.publish(2, 0xc0)));
    steps.push(Step::Acquired(backend.acquire_and_hold()));

    // Lease the whole ring.
    steps.push(Step::Published(backend.publish(3, 0xd0)));
    steps.push(Step::Acquired(backend.acquire_and_hold()));
    steps.push(Step::Published(backend.publish(4, 0xe0)));
    steps.push(Step::Acquired(backend.acquire_and_hold()));

    // The renderer does not stall; the publication is dropped and counted.
    steps.push(Step::Published(backend.publish(5, 0xf0)));
    steps.push(Step::Dropped(backend.dropped()));

    // A returned lease makes the slot reusable again.
    backend.release_one();
    steps.push(Step::Published(backend.publish(6, 0x11)));
    steps.push(Step::Dropped(backend.dropped()));

    steps
}

/// The shared-memory transport reproduces the in-process ring's publication
/// semantics exactly, driven through the shared header instead of a mutex.
#[test]
fn transport_shm_parity() {
    let mut cpu = CpuBackend {
        ring: PublicationRing::new(layers(), EXTENT, RING, PublishMode::Commit)
            .expect("a legal ring"),
        held: Vec::new(),
    };

    let driver = ShmDriver::create(&layers(), EXTENT, RING).expect("a segment");
    let mut shm = ShmBackend {
        client: ShmClient::new(driver.segment().clone()),
        driver,
        held: Vec::new(),
    };

    let cpu_trace = scenario(&mut cpu);
    let shm_trace = scenario(&mut shm);

    assert_eq!(
        cpu_trace, shm_trace,
        "the shm transport must preserve the in-process semantics"
    );

    // Spot-check the trace itself, so parity cannot be satisfied by two
    // identically broken implementations.
    assert_eq!(cpu_trace[0], Step::Acquired(None));
    assert_eq!(cpu_trace[2], Step::Acquired(Some((1, 0, 0xa0))));
    assert_eq!(cpu_trace[3], Step::Acquired(None));
    assert_eq!(cpu_trace[6], Step::Acquired(Some((3, 2, 0xc0))));
    assert_eq!(cpu_trace[11], Step::Published(None));
    assert_eq!(cpu_trace[12], Step::Dropped(1));
    assert!(matches!(cpu_trace[13], Step::Published(Some(_))));
    assert_eq!(cpu_trace[14], Step::Dropped(1));

    // The client sees the driver's close through the header.
    assert!(!shm.client.is_drained());
    let final_value = shm.driver.close();
    assert_eq!(final_value, 7, "one past the last frame serial");
    assert!(matches!(shm.driver.begin_write(), Err(Error::Closed)));

    while !shm.held.is_empty() {
        shm.release_one();
    }

    assert!(shm.client.is_drained());
}

// ─── Client Loss ────────────────────────────────────────────────────────────

/// A unique rendezvous path for this process.
fn channel_path() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let path = std::env::temp_dir().join(format!(
        "nsi-stream-{}-{}.sock",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::AcqRel)
    ));
    let _ = std::fs::remove_file(&path);

    path
}

/// Run the open handshake, then kill the client end and report what the
/// driver did with the next publications.
fn after_client_loss(on_client_loss: ClientLoss) -> (bool, bool, u64, usize) {
    let path = channel_path();
    let listener = ClientChannel::bind(&path).expect("the client binds");
    let (ready, opened) = mpsc::channel();

    // The client: accept, read the handshake, then vanish.
    let client = thread::spawn(move || {
        let mut session = listener.accept().expect("the driver connects");

        let hello = session.recv().expect("a Hello frame");
        assert!(matches!(hello, Message::Hello { version: 1 }));

        let open = session.recv().expect("an Open frame");
        let planes = match open {
            Message::Open {
                extent,
                layers,
                ring,
                transport,
                fd,
            } => {
                assert_eq!(extent, EXTENT);
                assert_eq!(ring, RING);
                assert_eq!(transport, Transport::Shm);
                assert_eq!(layers.len(), 1);
                assert_eq!(layers[0].name, "beauty");
                assert_eq!(layers[0].variable_name, "Ci");
                assert_eq!(layers[0].format, LayerFormat::RgbaF16);

                // The descriptor really is the driver's segment.
                let segment = ShmClient::attach(fd.expect("a passed handle"))
                    .expect("the segment maps");
                assert_eq!(segment.segment().extent(), EXTENT);
                assert_eq!(segment.segment().ring_size(), RING);

                segment.segment().layers().len()
            }
            other => panic!("expected `Open`, got {other:?}"),
        };

        ready.send(planes).expect("the driver waits");

        // Dropping the session closes the client end of the socket.
    });

    let mut driver = DriverChannel::connect(&path, on_client_loss)
        .expect("the driver connects");
    let segment =
        ShmDriver::create(&layers(), EXTENT, RING).expect("a segment");

    driver.send_hello(1).expect("Hello is sent");
    driver
        .send_open(
            &OpenMessage {
                extent: EXTENT,
                layers: layers(),
                ring: RING,
                transport: Transport::Shm,
            },
            Some(segment.segment().as_fd()),
        )
        .expect("Open is sent");

    let planes = opened.recv().expect("the client got the handshake");
    client.join().expect("the client thread");

    // The client is gone. Publish until the loss is detected.
    let mut publication = segment
        .publish(
            segment
                .begin_write()
                .expect("open segment")
                .expect("a free slot"),
            0,
        )
        .expect("a publication");
    let mut stopped = false;

    (0..16).for_each(|_| {
        if !driver.client_lost() && !stopped {
            publication.frame_serial += 1;

            if driver.send_publish(&publication).is_err() {
                stopped = true;
            }

            thread::sleep(Duration::from_millis(1));
        }
    });

    (
        driver.client_lost(),
        driver.should_stop(),
        driver.dropped(),
        planes,
    )
}

/// When the client vanishes the driver detects it and honors
/// `stream.onclientloss`: `"continue"` keeps rendering and counts the
/// dropped publications, `"stop"` raises the stop flag.
#[test]
fn client_loss_behavior() {
    let (lost, stop, dropped, planes) = after_client_loss(ClientLoss::Continue);

    assert_eq!(planes, 1, "the handshake carried the segment");
    assert!(lost, "the driver must notice the client is gone");
    assert!(!stop, "`continue` must not stop the renderer");
    assert!(
        dropped > 0,
        "publications after the loss are dropped and counted"
    );

    let (lost, stop, _, _) = after_client_loss(ClientLoss::Stop);

    assert!(lost, "the driver must notice the client is gone");
    assert!(stop, "`stop` must raise the stop flag");
}
