//! Contract: `contracts/attribute-vocabulary.md`, negotiation rows.
//!
//! - "`stream.transport \"auto\"` negotiates gpu → shm → callback and
//!   reports the selected transport" -- `transport_auto_negotiation`.
//! - "Explicit transport that is unviable fails open() with a typed error,
//!   no fallback" -- `transport_explicit_no_fallback`.
//! - "`stream.device.uuid` mismatch fails/falls back per transport rules"
//!   -- `transport_device_mismatch`.

use nsi_stream::{
    Error, Extent, Layer, LayerFormat, StaticProbe, StreamConfig, StreamDriver,
    Transport, TransportRequest, negotiate,
};

const DEVICE: &str = "0123abcd-0000-0000-0000-000000000000";
const OTHER_DEVICE: &str = "ffffffff-1111-2222-3333-444444444444";

fn requesting(transport: TransportRequest) -> StreamConfig {
    StreamConfig {
        transport,
        ..StreamConfig::default()
    }
}

fn open_with(
    config: StreamConfig,
    probe: &StaticProbe,
) -> Result<StreamDriver, Error> {
    StreamDriver::open(
        config,
        vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16)],
        Extent::new(64, 64),
        probe,
    )
}

/// `"auto"` walks gpu → shm → callback and reports what it selected. With
/// every candidate unviable it fails with a typed error rather than opening
/// something that cannot carry pixels.
#[test]
fn transport_auto_negotiation() {
    let config = requesting(TransportRequest::Auto);

    // All viable: the GPU transport wins.
    assert_eq!(
        negotiate(&config, &StaticProbe::all_viable())
            .expect("a viable transport"),
        Transport::GpuShared
    );

    // GPU forced unviable: falls through to shared memory.
    let no_gpu = StaticProbe::all_viable()
        .unviable(Transport::GpuShared, "no Vulkan loader on this host");

    assert_eq!(
        negotiate(&config, &no_gpu).expect("a viable transport"),
        Transport::Shm
    );
    assert_eq!(
        open_with(config.clone(), &no_gpu)
            .expect("open succeeds on the fallback")
            .transport(),
        Transport::Shm
    );

    // GPU and shm unviable: falls through to the in-process callbacks.
    let callback_only = no_gpu
        .clone()
        .unviable(Transport::Shm, "built without the `shm` feature");

    assert_eq!(
        negotiate(&config, &callback_only).expect("a viable transport"),
        Transport::Callback
    );

    // Nothing viable at all: typed error, no partial open.
    let nothing = callback_only.unviable(Transport::Callback, "out of process");

    assert!(matches!(
        negotiate(&config, &nothing),
        Err(Error::TransportUnavailable { ref transport, .. })
            if transport == "auto"
    ));
    assert!(matches!(
        open_with(config, &nothing),
        Err(Error::TransportUnavailable { .. })
    ));
}

/// An explicitly named transport never falls back -- not even to a transport
/// that is viable and would have been chosen under `"auto"`.
#[test]
fn transport_explicit_no_fallback() {
    // `"gpu"` requested, gpu unviable, everything else viable.
    let probe = StaticProbe::all_viable()
        .unviable(Transport::GpuShared, "no Vulkan loader on this host");
    let config = requesting(TransportRequest::Explicit(Transport::GpuShared));

    let error = negotiate(&config, &probe)
        .expect_err("an explicit transport must not fall back");

    assert!(matches!(
        error,
        Error::TransportUnavailable { ref transport, ref reason }
            if transport == "gpu"
                && reason.contains("no Vulkan loader")
    ));
    assert!(matches!(
        open_with(config, &probe),
        Err(Error::TransportUnavailable { .. })
    ));

    // The same for `"shm"` and `"callback"`.
    [Transport::Shm, Transport::Callback]
        .into_iter()
        .for_each(|transport| {
            let probe =
                StaticProbe::all_viable().unviable(transport, "fixture");

            assert!(matches!(
                negotiate(
                    &requesting(TransportRequest::Explicit(transport)),
                    &probe
                ),
                Err(Error::TransportUnavailable { .. })
            ));
        });
}

/// A `stream.device.uuid` the driver does not render on is a hard error for
/// an explicit `"gpu"` request, and makes the GPU transport non-viable (so
/// negotiation falls through) under `"auto"`.
#[test]
fn transport_device_mismatch() {
    let probe = StaticProbe::all_viable().with_device_uuid(OTHER_DEVICE);

    let explicit = StreamConfig {
        transport: TransportRequest::Explicit(Transport::GpuShared),
        device_uuid: Some(DEVICE.to_string()),
        ..StreamConfig::default()
    };

    assert_eq!(
        negotiate(&explicit, &probe)
            .expect_err("an explicit device is never substituted"),
        Error::DeviceMismatch {
            requested: DEVICE.to_string(),
            actual: OTHER_DEVICE.to_string(),
        }
    );
    assert!(matches!(
        open_with(explicit, &probe),
        Err(Error::DeviceMismatch { .. })
    ));

    // Under `"auto"` the mismatch only disqualifies the GPU transport.
    let auto = StreamConfig {
        device_uuid: Some(DEVICE.to_string()),
        ..StreamConfig::default()
    };

    assert_eq!(
        negotiate(&auto, &probe).expect("negotiation falls through"),
        Transport::Shm
    );
    assert_eq!(
        open_with(auto.clone(), &probe)
            .expect("open succeeds on the fallback")
            .transport(),
        Transport::Shm
    );

    // A matching UUID keeps the GPU transport.
    assert_eq!(
        negotiate(&auto, &StaticProbe::all_viable().with_device_uuid(DEVICE))
            .expect("a matching adapter"),
        Transport::GpuShared
    );
}

/// The GPU transport reports cleanly -- and this test skips -- when the host
/// has no Vulkan loader or ICD.
#[cfg(feature = "vulkan")]
#[test]
fn gpu_runtime_skips_without_vulkan() {
    use nsi_stream::transport::gpu;

    match gpu::probe() {
        Err(Error::TransportUnavailable { reason, .. }) => {
            println!("skipped: no vulkan device ({reason})");
        }
        Err(other) => panic!("a missing loader must be typed, got {other:?}"),
        Ok(()) => {
            let context =
                gpu::VulkanContext::open(None).expect("a device is present");
            let timeline = gpu::VulkanTimeline::new(&context)
                .expect("a timeline semaphore");

            timeline.signal(1).expect("host signal");
            assert_eq!(timeline.value().expect("counter"), 1);
        }
    }
}
