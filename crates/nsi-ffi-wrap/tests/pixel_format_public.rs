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

/// Known defect: indexed channels (ndspy's native format) self-trigger layer boundaries.
///
/// Root cause: "s" (scalar) appears in both the layer-ender set ["b","z","s","a"]
/// and the layer-starter set ["r","x","s"], causing a single "s" channel to match
/// both patterns and incorrectly trigger a boundary emission.
///
/// Current behavior: `["beauty.000"]` produces 2 duplicate layers, each
/// {"beauty", OneChannel}, so channels()==2.
/// Correct behavior: should produce 1 layer {"beauty", OneChannel}, channels()==1.
#[test]
#[ignore]
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
/// Total channels()==4.
#[test]
#[ignore]
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
