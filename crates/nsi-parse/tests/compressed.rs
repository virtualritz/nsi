//! Compressed input, detected rather than declared.
#![cfg(any(feature = "gzip", feature = "zstd"))]

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{Compression, Recorder, write_stream_with};
use nsi_parse::parse_compressed;
use nsi_trait::Nsi;

fn scene() -> nsi_intermediate::Scene {
    let recorder = Recorder::new();
    recorder.create("m", "mesh", None).unwrap();
    recorder
        .set_attribute("m", &[nsi::f32!("fov", 45.0)])
        .unwrap();
    recorder
        .connect("m", None, ".root", "objects", None)
        .unwrap();
    recorder.into_scene()
}

fn written(compression: Compression) -> Vec<u8> {
    let mut out = Vec::new();
    write_stream_with(&scene(), &mut out, compression).expect("write");
    out
}

fn parses_to_two_nodes(input: &[u8]) {
    let recorder = Recorder::new();
    parse_compressed(input, &recorder).expect("parse");
    let parsed = recorder.into_scene();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed.edges().count(), 1);
}

/// Plain input goes straight through, so a caller need not know.
#[test]
fn uncompressed_input_is_passed_through() {
    parses_to_two_nodes(&written(Compression::None));
}

#[cfg(feature = "gzip")]
#[test]
fn gzip_is_detected_and_read() {
    let packed = written(Compression::Gzip);
    assert_eq!(&packed[..2], &[0x1f, 0x8b], "actually gzip");
    parses_to_two_nodes(&packed);
}

#[cfg(feature = "zstd")]
#[test]
fn zstd_is_detected_and_read() {
    let packed = written(Compression::Zstd);
    assert_eq!(&packed[..4], &[0x28, 0xb5, 0x2f, 0xfd], "actually zstd");
    parses_to_two_nodes(&packed);
}

/// Announcing a compressor and then not being one is an error, not a
/// stream that happens to fail to parse.
#[cfg(feature = "gzip")]
#[test]
fn a_truncated_compressed_stream_is_an_error() {
    let mut packed = written(Compression::Gzip);
    packed.truncate(packed.len() / 2);

    let recorder = Recorder::new();
    let error = parse_compressed(&packed, &recorder).expect_err("must fail");
    assert!(
        matches!(error, nsi_parse::Error::Decompress(_)),
        "got {error:?}"
    );
}
