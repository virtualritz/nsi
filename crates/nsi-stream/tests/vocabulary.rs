//! Contract: `contracts/attribute-vocabulary.md`.
//!
//! Rows covered here:
//!
//! - "Driver is addressed via `drivername \"nsi-stream\"`; all `stream.*`
//!   attributes reach it unmodified" -- `vocabulary_forwarding`.
//! - "Missing `stream.version` or unsupported version fails open() with a
//!   typed error" -- `vocabulary_version_reject`.
//! - "Unknown `stream.*` attribute is ignored with a warning, not an error"
//!   -- `vocabulary_unknown_attr_warns`.
//! - "Per-layer format from each connected `outputlayer` is honored (RGBA
//!   f16/f32 minimum)" -- `layer_formats`.

use nsi_stream::{
    Attr, Bucket, ClientLoss, DRIVER_NAME, Error, Extent, Layer, LayerFormat,
    PublicationRing, PublishMode, StaticProbe, StreamConfig, StreamDriver,
    Transport, TransportRequest,
};
use std::ffi::c_void;

/// The full version-1 attribute set as a client would set it on the
/// `outputdriver` node, including an unknown `stream.*` attribute and two
/// attributes outside the namespace.
fn full_attribute_set() -> Vec<Attr> {
    vec![
        // Not part of the `stream.` namespace -- forwarded elsewhere,
        // ignored here without a warning.
        Attr::string("drivername", DRIVER_NAME),
        Attr::string("imagefilename", "unused.exr"),
        Attr::int("stream.version", 1),
        Attr::string("stream.transport", "shm"),
        Attr::string("stream.publish", "continuous"),
        Attr::int("stream.ring", 4),
        Attr::string("stream.channel", "/tmp/nsi-stream.sock"),
        Attr::string(
            "stream.device.uuid",
            "0123abcd-0000-0000-0000-000000000000",
        ),
        Attr::string("stream.onclientloss", "stop"),
        Attr::pointer("stream.callback.open", 0x1000 as *const c_void),
        Attr::pointer("stream.callback.publish", 0x2000 as *const c_void),
        Attr::pointer("stream.callback.close", 0x3000 as *const c_void),
        // A vocabulary from a later version: a warning, never an error.
        Attr::int("stream.lookahead", 2),
    ]
}

/// Every `stream.*` attribute reaches the driver and decodes into the
/// documented `StreamConfig` field; non-`stream.*` attributes are ignored.
#[test]
fn vocabulary_forwarding() {
    let (config, warnings) =
        StreamConfig::parse(&full_attribute_set()).expect("version 1 parses");

    assert_eq!(config.version, 1);
    assert_eq!(config.transport, TransportRequest::Explicit(Transport::Shm));
    assert_eq!(config.publish, PublishMode::Continuous);
    assert_eq!(config.ring, 4);
    assert_eq!(config.channel.as_deref(), Some("/tmp/nsi-stream.sock"));
    assert_eq!(
        config.device_uuid.as_deref(),
        Some("0123abcd-0000-0000-0000-000000000000")
    );
    assert_eq!(config.on_client_loss, ClientLoss::Stop);
    assert_eq!(
        config.callbacks.open.map(|pointer| pointer.as_ptr()),
        Some(0x1000 as *const c_void)
    );
    assert_eq!(
        config.callbacks.publish.map(|pointer| pointer.as_ptr()),
        Some(0x2000 as *const c_void)
    );
    assert_eq!(
        config.callbacks.close.map(|pointer| pointer.as_ptr()),
        Some(0x3000 as *const c_void)
    );

    // Exactly one warning: the unknown `stream.*` attribute. `drivername`
    // and `imagefilename` are outside the namespace and are not this
    // parser's business.
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].attribute, "stream.lookahead");
}

/// A missing version and an unsupported version both fail with a typed
/// error, and neither is defaulted.
#[test]
fn vocabulary_version_reject() {
    let missing = StreamConfig::parse(&[
        Attr::string("drivername", "nsi-stream"),
        Attr::string("stream.transport", "auto"),
    ])
    .expect_err("a missing version is fatal");

    assert_eq!(
        missing,
        Error::MissingAttribute {
            name: "stream.version".to_string()
        }
    );

    let unsupported = StreamConfig::parse(&[
        Attr::string("drivername", "nsi-stream"),
        Attr::int("stream.version", 2),
    ])
    .expect_err("version 2 is not implemented");

    assert_eq!(
        unsupported,
        Error::UnsupportedVersion {
            requested: 2,
            supported: 1
        }
    );

    // The version gate also guards `open()`, for a config built by hand.
    let config = StreamConfig {
        version: 2,
        ..StreamConfig::default()
    };

    assert_eq!(
        StreamDriver::open(
            config,
            vec![Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16)],
            Extent::new(16, 16),
            &StaticProbe::all_viable(),
        )
        .expect_err("open rejects the version too"),
        Error::UnsupportedVersion {
            requested: 2,
            supported: 1
        }
    );
}

/// An unknown `stream.*` attribute is listed as a warning; the parse
/// succeeds and the rest of the table is honored.
#[test]
fn vocabulary_unknown_attr_warns() {
    let (config, warnings) = StreamConfig::parse(&[
        Attr::int("stream.version", 1),
        Attr::int("stream.ring", 5),
        Attr::string("stream.faster", "yes"),
        Attr::int("stream.tiles", 64),
    ])
    .expect("unknown attributes never fail the parse");

    assert_eq!(config.ring, 5);

    let named = warnings
        .iter()
        .map(|warning| warning.attribute.as_str())
        .collect::<Vec<_>>();

    assert_eq!(named, vec!["stream.faster", "stream.tiles"]);
    assert!(warnings[0].to_string().contains("stream.faster"));
}

/// Each layer's declared format is honored in the slot storage: an f32 layer
/// occupies twice the bytes of the f16 layer of the same extent, and each
/// layer is individually addressable by its plane index.
#[test]
fn layer_formats() {
    let extent = Extent::new(32, 16);
    let layers = vec![
        Layer::rgba("beauty", "Ci", LayerFormat::RgbaF16),
        Layer::rgba("ids", "id.object", LayerFormat::RgbaF32),
        Layer::new("alpha", "a", LayerFormat::RgbaF32, 1),
    ];

    let ring =
        PublicationRing::new(layers.clone(), extent, 2, PublishMode::Commit)
            .expect("a legal ring");

    // Tag each layer with a distinct value so the planes can be told apart.
    layers.iter().enumerate().for_each(|(index, _)| {
        ring.fill_bucket(index, Bucket::full(extent), 0x10 + index as u8)
            .expect("a full-extent bucket");
    });

    let publication = ring.commit(0).expect("open ring").expect("a free slot");

    assert_eq!(publication.extent, extent);

    let token = ring.acquire().expect("the publication");

    // Sizes follow the declared formats.
    assert_eq!(
        token.plane(0).expect("the f16 beauty plane").len(),
        32 * 16 * 4 * 2
    );
    assert_eq!(
        token.plane(1).expect("the f32 id plane").len(),
        32 * 16 * 4 * 4
    );
    assert_eq!(
        token.plane(2).expect("the single-channel f32 plane").len(),
        32 * 16 * 4
    );
    assert!(token.plane(3).is_none());

    // Each layer is individually addressable and carries its own pixels.
    layers.iter().enumerate().for_each(|(index, _)| {
        assert!(
            token
                .plane(index)
                .expect("a declared plane")
                .iter()
                .all(|byte| *byte == 0x10 + index as u8),
            "layer {index} must carry only its own pixels"
        );
    });

    ring.release(token);
}
