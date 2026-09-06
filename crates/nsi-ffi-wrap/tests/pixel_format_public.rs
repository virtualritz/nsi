//! `PixelFormat` must be constructible by out-of-crate display drivers.
use nsi_ffi_wrap::output::PixelFormat;
use std::ffi::CString;

#[test]
fn a_display_driver_can_build_a_pixel_format_from_ndspy() {
    let name = CString::new("beauty.r").unwrap();
    let format = [ndspy_sys::PtDspyDevFormat {
        name: name.as_ptr(),
        type_: 1, // PkDspyFloat32
    }];

    let pixel_format = PixelFormat::from_ndspy(&format);

    assert_eq!(1, pixel_format.len());
    assert_eq!(1, pixel_format.channels());
}

/// An RGB layer with no alpha -- common, and it used to yield an *empty*
/// `PixelFormat`.
///
/// This pins `channels()`, which the `nsi-display` shim uses as a slice length
/// over the renderer's bucket buffer: a wrong count there is an out-of-bounds
/// read, so the value is memory-safety-critical and must not drift.
#[test]
fn rgb_without_alpha_is_one_three_channel_layer() {
    let names = [
        CString::new("Ci.r").unwrap(),
        CString::new("Ci.g").unwrap(),
        CString::new("Ci.b").unwrap(),
    ];
    let format = [
        ndspy_sys::PtDspyDevFormat {
            name: names[0].as_ptr(),
            type_: 1, // PkDspyFloat32
        },
        ndspy_sys::PtDspyDevFormat {
            name: names[1].as_ptr(),
            type_: 1,
        },
        ndspy_sys::PtDspyDevFormat {
            name: names[2].as_ptr(),
            type_: 1,
        },
    ];

    let pixel_format = PixelFormat::from_ndspy(&format);

    assert_eq!(1, pixel_format.len(), "should produce 1 layer");
    assert_eq!(3, pixel_format.channels(), "should have 3 channels");
}

/// Regression: indexed channels (ndspy's native format) used to self-trigger
/// layer boundaries.
///
/// Root cause was that "s" (scalar) appears in both the layer-ender set
/// ["b","z","s","a"] and the layer-starter set ["r","x","s"]. The first loop
/// step compares the seeded `previous_*` against the very same entry, so a lone
/// "s" matched both patterns and emitted a spurious duplicate layer:
/// `["beauty.000"]` produced 2 layers, channels()==2.
///
/// A single `PtDspyDevFormat` entry must never yield more than one layer, and
/// the display shim uses `channels()` as a slice length, so an inflated count
/// is a memory-safety hazard, not just a cosmetic one.
#[test]
fn indexed_channels_self_trigger_boundary() {
    let name = CString::new("beauty.000").unwrap();
    let format = [ndspy_sys::PtDspyDevFormat {
        name: name.as_ptr(),
        type_: 1, // PkDspyFloat32
    }];

    let pixel_format = PixelFormat::from_ndspy(&format);

    // Correct expected values:
    assert_eq!(1, pixel_format.len(), "should produce 1 layer");
    assert_eq!(1, pixel_format.channels(), "should have 1 channel");
}

/// Known defect: layers whose first channel is not in ["r","x","s"] never
/// trigger a boundary, causing them to be silently dropped and their channels
/// to be misattributed to the previous layer.
///
/// Root cause: the boundary heuristic assumes layer transitions occur when
/// transitioning from an ender ["b","z","s","a"] to a starter ["r","x","s"].
/// But a layer starting with "z" (vector z-component) will never match the
/// starter set, so it gets merged with the preceding layer.
///
/// Current behavior: `["Ci.r","Ci.g","Ci.b","depth.z"]` produces 1 layer
/// {"depth", Color, 0}, channels()==3. The "depth.z" becomes Color (3 channels)
/// incorrectly, and the Ci layer is dropped.
/// Correct behavior: should produce 2 layers:
/// - {"Ci", Color, 0} with channels 0-2
/// - {"depth", OneChannel, 3} with channel 3
///
/// Total channels()==4.
#[test]
fn layers_starting_with_z_are_silently_dropped() {
    let names = [
        CString::new("Ci.r").unwrap(),
        CString::new("Ci.g").unwrap(),
        CString::new("Ci.b").unwrap(),
        CString::new("depth.z").unwrap(),
    ];
    let format = [
        ndspy_sys::PtDspyDevFormat {
            name: names[0].as_ptr(),
            type_: 1,
        },
        ndspy_sys::PtDspyDevFormat {
            name: names[1].as_ptr(),
            type_: 1,
        },
        ndspy_sys::PtDspyDevFormat {
            name: names[2].as_ptr(),
            type_: 1,
        },
        ndspy_sys::PtDspyDevFormat {
            name: names[3].as_ptr(),
            type_: 1,
        },
    ];

    let pixel_format = PixelFormat::from_ndspy(&format);

    // Correct expected values:
    assert_eq!(2, pixel_format.len(), "should produce 2 layers");
    assert_eq!(4, pixel_format.channels(), "should have 4 channels total");
}

/// Builds a `PixelFormat` from channel names, all `PkDspyFloat32`.
fn parse(names: &[&str]) -> PixelFormat {
    let owned: Vec<CString> =
        names.iter().map(|n| CString::new(*n).unwrap()).collect();
    let format: Vec<ndspy_sys::PtDspyDevFormat> = owned
        .iter()
        .map(|n| ndspy_sys::PtDspyDevFormat {
            name: n.as_ptr(),
            type_: 1, // PkDspyFloat32
        })
        .collect();
    PixelFormat::from_ndspy(&format)
}

/// The invariant the display shim's memory safety rests on: ndspy hands
/// one `PtDspyDevFormat` per channel, so the parsed total must equal the
/// number of entries -- never more (an out-of-bounds read in
/// `nsi-display`'s `shim_data`) and never fewer (silently dropped AOVs).
///
/// Stated over every shape the parser is expected to meet, so a future
/// heuristic cannot satisfy one case by breaking another.
#[test]
fn channel_count_always_equals_the_number_of_format_entries() {
    for names in [
        vec!["Ci.r", "Ci.g", "Ci.b"],
        vec!["Ci.r", "Ci.g", "Ci.b", "Ci.a"],
        vec!["r", "g", "b", "a"],
        vec!["beauty.000"],
        vec!["depth.z"],
        vec!["Ci.r", "Ci.g", "Ci.b", "depth.z"],
        vec!["N.x", "N.y", "N.z"],
        vec!["Ci.r", "Ci.g", "Ci.b", "Ci.a", "N.x", "N.y", "N.z"],
        vec!["albedo.r", "albedo.g", "albedo.b", "depth.z"],
    ] {
        let pixel_format = parse(&names);
        assert_eq!(
            names.len(),
            pixel_format.channels(),
            "one channel per format entry, for {names:?}"
        );
    }
}

/// The format an OIDN-denoising EXR driver actually receives: beauty
/// plus the three utility passes. Every layer must survive with its own
/// name and offset, or the driver cannot tell which passes it was given.
#[test]
fn a_denoise_ready_multi_aov_format_keeps_every_layer() {
    let pixel_format = parse(&[
        "Ci.r", "Ci.g", "Ci.b", "Ci.a", // beauty, rgba
        "albedo.r", "albedo.g", "albedo.b", // albedo, rgb
        "N.x", "N.y", "N.z", // normal, xyz
        "depth.z", // depth, scalar
    ]);

    let layers: Vec<(&str, usize, usize)> = pixel_format
        .iter()
        .map(|l| (l.name(), l.channels(), l.offset()))
        .collect();

    assert_eq!(
        vec![
            ("Ci", 4, 0),
            ("albedo", 3, 4),
            ("N", 3, 7),
            ("depth", 1, 10),
        ],
        layers
    );
    assert_eq!(11, pixel_format.channels());
}

/// A bare `a` after a named layer is that layer's alpha, not a layer of
/// its own -- the one case where a change of layer name is not a
/// boundary.
#[test]
fn a_lone_alpha_belongs_to_the_layer_before_it() {
    let pixel_format = parse(&["Ci.r", "Ci.g", "Ci.b", "a"]);

    assert_eq!(1, pixel_format.len(), "the alpha joins Ci");
    assert_eq!(4, pixel_format.channels());
    assert!(pixel_format[0].has_alpha());
}
