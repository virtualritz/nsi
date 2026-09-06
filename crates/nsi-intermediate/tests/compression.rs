//! Compressed streams must be readable -- by a decompressor, and by
//! 3Delight itself.
#![cfg(any(feature = "gzip", feature = "zstd"))]

use nsi_ffi_wrap as nsi;
use nsi_intermediate::{
    Compression, Recorder, write_stream, write_stream_with,
};
use nsi_trait::Nsi;
use std::process::Command;

fn scene() -> nsi_intermediate::Scene {
    let recorder = Recorder::new();
    recorder.create("cam", "perspectivecamera", None).unwrap();
    recorder
        .set_attribute("cam", &[nsi::f32!("fov", 45.0)])
        .unwrap();
    recorder.create("m", "mesh", None).unwrap();
    let points = [[0.0f32, 0.0, 0.0], [1.0, 2.0, 3.0]];
    recorder
        .set_attribute("m", &[nsi::point_slice!("P", &points)])
        .unwrap();
    recorder
        .connect("m", None, ".root", "objects", None)
        .unwrap();
    recorder.into_scene()
}

fn plain() -> Vec<u8> {
    let mut out = Vec::new();
    write_stream(&scene(), &mut out).expect("write_stream");
    out
}

fn compressed(compression: Compression) -> Vec<u8> {
    let mut out = Vec::new();
    write_stream_with(&scene(), &mut out, compression).expect("compressed");
    out
}

/// A compressed stream must decompress to exactly the plain one.
/// Compression is a property of the file, not of the format.
#[cfg(feature = "gzip")]
#[test]
fn gzip_decompresses_to_the_plain_stream() {
    use std::io::Read;
    let packed = compressed(Compression::Gzip);
    assert_ne!(packed, plain(), "actually compressed");

    let mut unpacked = Vec::new();
    flate2::read::GzDecoder::new(packed.as_slice())
        .read_to_end(&mut unpacked)
        .expect("gzip round trip");
    assert_eq!(unpacked, plain());
}

#[cfg(feature = "zstd")]
#[test]
fn zstd_decompresses_to_the_plain_stream() {
    let packed = compressed(Compression::Zstd);
    assert_ne!(packed, plain(), "actually compressed");
    let unpacked =
        zstd::decode_all(packed.as_slice()).expect("zstd round trip");
    assert_eq!(unpacked, plain());
}

#[test]
fn the_extension_names_the_compressor() {
    assert_eq!(Compression::None.extension(), "");
    #[cfg(feature = "gzip")]
    assert_eq!(Compression::Gzip.extension(), ".gz");
    #[cfg(feature = "zstd")]
    assert_eq!(Compression::Zstd.extension(), ".zst");
}

/// The one that matters: 3Delight must read the file we wrote. A stream
/// that only our own decompressor accepts is not an ɴsɪ stream.
#[cfg(feature = "gzip")]
#[test]
fn the_renderer_reads_a_gzipped_stream() {
    let Ok(root) = std::env::var("DELIGHT") else {
        panic!("DELIGHT must point at a 3Delight install for this gate");
    };
    let renderdl = std::path::Path::new(&root).join("bin").join("renderdl");

    let path = std::env::temp_dir().join("nsi-intermediate-gate.nsi.gz");
    std::fs::write(&path, compressed(Compression::Gzip)).expect("written");

    let output = Command::new(&renderdl)
        .arg("-cat")
        .arg(&path)
        .output()
        .expect("renderdl ran");
    let replayed = String::from_utf8_lossy(&output.stdout);

    for statement in ["Create \"cam\"", "Create \"m\"", "Connect \"m\""] {
        assert!(
            replayed.contains(statement),
            "3Delight did not read the gzipped stream ({statement} missing):\n\
             --- stdout ---\n{replayed}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
