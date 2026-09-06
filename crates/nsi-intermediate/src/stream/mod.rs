//! `.nsi` stream emission.
//!
//! Replays a recorded scene in the ɴsɪ stream format so it can be
//! compared against what 3Delight writes for the same calls. Nodes are
//! emitted in creation order, which is why the tables are `IndexMap`s.
//!
//! The format below was read off 3Delight 2.9.207's own `apistream`
//! output rather than inferred, which is how the less obvious details
//! got settled: `I64` is `int64`, `MatrixF64` is `doublematrix`, and an
//! array length rides *inside* the type name as `int[2]` rather than in
//! a field of its own.
//!
//! ```text
//! Create "cam" "perspectivecamera"
//! SetAttribute "cam"
//!   "fov" "float" 1 45
//! SetAttribute "m"
//!   "P" "point" 2 [ 0 0 0 1 2 3 ]
//!   "resolution" "int[2]" 1 [ 1280 720 ]
//! SetAttributeAtTime "xf" 0.5
//!   "t" "double" 1 1
//! Connect "xf" "" ".root" "objects"
//! ```
//!
//! # What this is not
//!
//! A recorder holds scene *state*, not a call log: `set_attribute` is a
//! map insert, so the grouping of attributes into calls is discarded.
//! One ɴsɪ call setting three attributes and three calls setting one
//! each record identically, and both replay as three statements here.
//!
//! So a stream diff against 3Delight compares scene state, not call
//! history. That is the right invariant for a backend — the renderer
//! only ever sees final values — but it means a comparison fixture has
//! to be built one attribute per call for the two to align literally.

use crate::{EdgeKind, OwnedArg, OwnedData, Scene};
use nsi_ffi_wrap::nsi_sys::NSIParamFlags;
use nsi_trait::Type;
use std::io::{self, Write};

/// One ɴsɪ stream string literal, escaped.
///
/// Without this a handle or value containing a quote and a newline
/// closes its literal and the rest parses as further statements. The
/// stream is a persisted, cross-language format, so that is a
/// correctness hole rather than a cosmetic one. 3Delight escapes the
/// same way.
fn quoted(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 2);
    out.push(b'"');
    for &byte in value {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\t' => out.extend_from_slice(b"\\t"),
            // Every other control byte is three-digit octal, which is
            // what 3Delight writes -- a carriage return as `\015`. Left
            // raw, it would end the statement.
            control if control < 0x20 => {
                out.extend_from_slice(format!("\\{control:03o}").as_bytes());
            }
            // A byte at or above 0x7f goes out raw, which is what
            // 3Delight writes: `renderdl -cat` echoes a Latin-1 file
            // name back unchanged rather than escaping it.
            _ => out.push(byte),
        }
    }
    out.push(b'"');
    out
}

/// [`quoted`] for an identifier, which the scene stores as text.
///
/// Escaping only ever substitutes ASCII sequences and copies every other
/// byte through, so UTF-8 in gives UTF-8 out. The fallback is
/// unreachable rather than a repair.
fn quoted_str(value: &str) -> String {
    String::from_utf8(quoted(value.as_bytes())).unwrap_or_else(|error| {
        String::from_utf8_lossy(error.as_bytes()).into_owned()
    })
}

/// Format an `f64` as 3Delight does: C's `printf("%.17g")`.
///
/// Rust's `Display` writes the shortest representation that round-trips;
/// C's `%g` writes a fixed number of significant digits and switches to
/// exponent form outside a range. They agree on `0.5` and disagree on
/// `0.1`, `1.0 / 3.0`, `1e-7` and `1e20`, so the difference is not
/// cosmetic and the stream gate only passed because the fixture used
/// values from the agreeing set.
fn format_f64(value: f64) -> String {
    const SIGNIFICANT: i32 = 17;

    if value == 0.0 {
        if value.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        }
    } else if !value.is_finite() {
        // C prints these lowercase and unsuffixed.
        if value.is_nan() {
            "nan".to_string()
        } else if value.is_sign_negative() {
            "-inf".to_string()
        } else {
            "inf".to_string()
        }
    } else {
        let scientific = format!("{:.*e}", (SIGNIFICANT - 1) as usize, value);
        let (mantissa, exponent) =
            scientific.split_once('e').expect("Rust always writes one");
        let exponent: i32 = exponent.parse().expect("an integer exponent");

        if !(-4..SIGNIFICANT).contains(&exponent) {
            format!(
                "{}e{}{:02}",
                trim_zeros(mantissa),
                if exponent < 0 { '-' } else { '+' },
                exponent.abs()
            )
        } else {
            let decimals = (SIGNIFICANT - 1 - exponent) as usize;
            trim_zeros(&format!("{value:.decimals$}"))
        }
    }
}

/// Format an `f32` as 3Delight does.
///
/// A *different* printer from the one for doubles, which was a surprise
/// worth writing down: a double is `%.17g` (`0.1` becomes
/// `0.10000000000000001`), while a float is the shortest form that
/// round-trips, written in whichever of decimal or exponent notation is
/// shorter -- `1e5`, `1e-7`, but `123456790` and `0.1`. The exponent
/// carries no `+` and no padding, where the double printer's does.
///
/// Rust's `Display` never chooses exponent notation, so `100000.0`
/// would come out as `100000` where 3Delight writes `1e5`.
fn format_f32(value: f32) -> String {
    if value.is_finite() {
        let decimal = format!("{value}");
        let exponent = format!("{value:e}");
        if exponent.len() < decimal.len() {
            exponent
        } else {
            decimal
        }
    } else if value.is_nan() {
        "nan".to_string()
    } else if value.is_sign_negative() {
        "-inf".to_string()
    } else {
        "inf".to_string()
    }
}

/// Drop a decimal fraction's trailing zeros, and the point with them.
fn trim_zeros(value: &str) -> String {
    if value.contains('.') {
        value
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    } else {
        value.to_string()
    }
}

/// How a written ɴsɪ stream is compressed.
///
/// A compressed stream decompresses to exactly the plain one, so this
/// is a property of the file rather than of the format.
///
/// **Only gzip is read by 3Delight.** `renderdl` reads a `.nsi.gz`
/// wherever it reads a `.nsi`; handed a zstd stream it fails with
/// `Invalid char`, and a context configured with
/// `streamcompression="zstd"` writes plain text. So
/// `Compression::Zstd` is for consumers of *this* crate -- archives,
/// caches, transport -- and a file written with it is not something the
/// renderer will read back.
///
/// Each variant beyond [`Compression::None`] costs a dependency and is
/// behind the feature of the same name, so a consumer that wants neither
/// pays for neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Compression {
    /// Write the stream as-is.
    #[default]
    None,
    /// gzip, which 3Delight reads.
    #[cfg(feature = "gzip")]
    Gzip,
    /// Zstandard.
    ///
    /// Not readable by 3Delight 2.9.207; see the type documentation.
    #[cfg(feature = "zstd")]
    Zstd,
}

impl Compression {
    /// The conventional file-name suffix, appended to `.nsi`.
    ///
    /// Empty for [`Compression::None`].
    pub const fn extension(self) -> &'static str {
        match self {
            Self::None => "",
            #[cfg(feature = "gzip")]
            Self::Gzip => ".gz",
            #[cfg(feature = "zstd")]
            Self::Zstd => ".zst",
        }
    }
}

/// Write `scene` as an ɴsɪ stream, compressed.
///
/// Takes the writer by value because a compressor owns and must finish
/// it; a half-written compressed stream is not a truncated file, it is
/// an unreadable one.
///
/// # Errors
///
/// Any write failure, and any compressor failure.
pub fn write_stream_with<W: Write>(
    scene: &Scene,
    mut out: W,
    compression: Compression,
) -> io::Result<()> {
    match compression {
        Compression::None => {
            write_stream(scene, &mut out)?;
            out.flush()
        }
        #[cfg(feature = "gzip")]
        Compression::Gzip => {
            let mut encoder = flate2::write::GzEncoder::new(
                out,
                flate2::Compression::default(),
            );
            write_stream(scene, &mut encoder)?;
            encoder.finish()?.flush()
        }
        #[cfg(feature = "zstd")]
        Compression::Zstd => {
            let mut encoder = zstd::stream::write::Encoder::new(out, 0)?;
            write_stream(scene, &mut encoder)?;
            encoder.finish()?.flush()
        }
    }
}

/// Write `scene` as an ɴsɪ stream.
pub fn write_stream<W: Write>(scene: &Scene, out: &mut W) -> io::Result<()> {
    for (handle, node) in scene.nodes() {
        // ɴsɪ's reserved handles "don't need to be created using
        // NSICreate", and 3Delight writes no `Create` for them.
        if !crate::is_reserved(handle) {
            writeln!(
                out,
                "Create {} {}",
                quoted_str(handle),
                quoted_str(&node.node_type)
            )?;
        }

        for arg in node.attrs.values() {
            writeln!(out, "SetAttribute {}", quoted_str(handle))?;
            write_arg(out, arg)?;
        }

        for (time, attrs) in &node.time_attrs {
            for arg in attrs.values() {
                writeln!(
                    out,
                    "SetAttributeAtTime {} {}",
                    quoted_str(handle),
                    format_f64(*time)
                )?;
                write_arg(out, arg)?;
            }
        }
    }

    for edge in scene.edges() {
        let (from_port, to_port) = match &edge.kind {
            EdgeKind::ShaderNetwork { from_port, to_port } => {
                (from_port.as_str(), to_port.as_str())
            }
            other => ("", other.to_attr()),
        };
        writeln!(
            out,
            "Connect {} {} {} {}",
            quoted_str(&edge.from),
            quoted_str(from_port),
            quoted_str(&edge.to),
            quoted_str(to_port)
        )?;
        // ɴsɪ emits connection arguments as indented parameter lines
        // under the `Connect`, exactly as for `SetAttribute`.
        for arg in &edge.args {
            write_arg(out, arg)?;
        }
    }

    Ok(())
}

/// Write one attribute line: two-space indent, name, type, count, data.
fn write_arg<W: Write>(out: &mut W, arg: &OwnedArg) -> io::Result<()> {
    // A host pointer has no stream representation. 3Delight omits the
    // whole parameter line, keeping the statement that carried it, so
    // writing a header with no value would be malformed where 3Delight
    // writes nothing at all.
    if matches!(arg.data, OwnedData::Reference(_)) {
        return Ok(());
    }

    write!(
        out,
        "  {} {} {} ",
        quoted_str(&arg.name),
        quoted_str(&type_name(arg)),
        element_count(arg)
    )?;

    // 3Delight leaves exactly one scalar bare and brackets everything
    // else -- an empty slice included, which it writes as `[ ]`.
    let scalars = scalar_count(arg);
    if scalars == 0 {
        // No values, so no separating space either.
        return writeln!(out, "[ ]");
    }
    if scalars != 1 {
        write!(out, "[ ")?;
    }

    match &arg.data {
        OwnedData::F32(v) => {
            let formatted: Vec<String> =
                v.iter().copied().map(format_f32).collect();
            write!(out, "{}", formatted.join(" "))?;
        }
        OwnedData::F64(v) => {
            let formatted: Vec<String> =
                v.iter().copied().map(format_f64).collect();
            write!(out, "{}", formatted.join(" "))?;
        }
        OwnedData::I32(v) => write_scalars(out, v)?,
        OwnedData::I64(v) => write_scalars(out, v)?,
        OwnedData::String(v) => {
            for (index, value) in v.iter().enumerate() {
                if index > 0 {
                    out.write_all(b" ")?;
                }
                out.write_all(&quoted(value))?;
            }
        }
        // Returned above; a `Reference` never reaches here.
        OwnedData::Reference(_) => {}
    }

    if scalars != 1 {
        write!(out, " ]")?;
    }
    writeln!(out)
}

fn write_scalars<W: Write, T: std::fmt::Display>(
    out: &mut W,
    values: &[T],
) -> io::Result<()> {
    let joined: Vec<String> = values
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    write!(out, "{}", joined.join(" "))
}

/// Total scalars stored, across every element.
fn scalar_count(arg: &OwnedArg) -> usize {
    match &arg.data {
        OwnedData::F32(v) => v.len(),
        OwnedData::F64(v) => v.len(),
        OwnedData::I32(v) => v.len(),
        OwnedData::I64(v) => v.len(),
        OwnedData::String(v) => v.len(),
        OwnedData::Reference(v) => v.len(),
    }
}

/// The stream `count` field: elements, then divided by `array_length`.
///
/// A two-point `P` is 2. A `resolution` of two `i32`s with
/// `array_len(2)` is 1, because the array length is carried by the type
/// name instead.
fn element_count(arg: &OwnedArg) -> usize {
    let per = components_per_element(arg.type_tag);
    scalar_count(arg) / per / arg.array_length.max(1)
}

const fn components_per_element(type_tag: Type) -> usize {
    match type_tag {
        Type::Color | Type::Point | Type::Vector | Type::Normal => 3,
        Type::MatrixF32 | Type::MatrixF64 => 16,
        _ => 1,
    }
}

/// ɴsɪ stream type name, with the array length appended when there is
/// one -- `int` becomes `int[2]` under `array_len(2)`.
fn type_name(arg: &OwnedArg) -> String {
    let base = base_type_name(arg.type_tag);
    // ɴsɪ marks an array with `NSIParamIsArray`, not by its length, and
    // `array_len(1)` is a real one-element array: 3Delight writes
    // `"al1" "float[1]" 2 [ 1 2 ]`. Keying on `> 1` dropped it.
    let sized = if arg.flags & NSIParamFlags::IsArray.bits() != 0 {
        format!("{base}[{}]", arg.array_length)
    } else {
        base.to_string()
    };

    let flags = flag_prefix(arg.flags);
    if flags.is_empty() {
        sized
    } else {
        format!("{flags} {sized}")
    }
}

/// ɴsɪ's argument flags, as the letters 3Delight prefixes to the type.
///
/// `per_vertex` is `"v point"`, and flags combine into one run of
/// letters: `per_vertex` plus `linear_interpolation` is `"vl float"`.
/// `IsArray` is excluded because the array length is already carried by
/// the `[n]` suffix.
fn flag_prefix(flags: i32) -> String {
    [
        (NSIParamFlags::PerFace, 'f'),
        (NSIParamFlags::PerVertex, 'v'),
        (NSIParamFlags::InterpolateLinear, 'l'),
    ]
    .iter()
    .filter(|(flag, _)| flags & flag.bits() != 0)
    .map(|(_, letter)| *letter)
    .collect()
}

/// Verified against 3Delight 2.9.207 apistream output.
const fn base_type_name(type_tag: Type) -> &'static str {
    match type_tag {
        Type::F32 => "float",
        Type::F64 => "double",
        Type::I32 => "int",
        Type::I64 => "int64",
        Type::String => "string",
        Type::Color => "color",
        Type::Point => "point",
        Type::Vector => "vector",
        Type::Normal => "normal",
        Type::MatrixF32 => "matrix",
        Type::MatrixF64 => "doublematrix",
        Type::Reference => "pointer",
        Type::Invalid => "invalid",
    }
}

#[cfg(test)]
mod tests;
