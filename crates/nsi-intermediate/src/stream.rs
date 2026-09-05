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
fn quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
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

/// Write `scene` as an ɴsɪ stream.
pub fn write_stream<W: Write>(scene: &Scene, out: &mut W) -> io::Result<()> {
    for (handle, node) in &scene.nodes {
        writeln!(out, "Create {} {}", quoted(handle), quoted(&node.node_type))?;

        for arg in node.attrs.values() {
            writeln!(out, "SetAttribute {}", quoted(handle))?;
            write_arg(out, arg)?;
        }

        for (time, attrs) in &node.time_attrs {
            for arg in attrs.values() {
                writeln!(
                    out,
                    "SetAttributeAtTime {} {}",
                    quoted(handle),
                    format_f64(*time)
                )?;
                write_arg(out, arg)?;
            }
        }
    }

    for edge in &scene.edges {
        let (from_port, to_port) = match &edge.kind {
            EdgeKind::ShaderNetwork { from_port, to_port } => {
                (from_port.as_str(), to_port.as_str())
            }
            other => ("", to_attr_of(other)),
        };
        writeln!(
            out,
            "Connect {} {} {} {}",
            quoted(&edge.from),
            quoted(from_port),
            quoted(&edge.to),
            quoted(to_port)
        )?;
        // ɴsɪ emits connection arguments as indented parameter lines
        // under the `Connect`, exactly as for `SetAttribute`.
        for arg in &edge.args {
            write_arg(out, arg)?;
        }
    }

    Ok(())
}

/// The ɴsɪ destination attribute an [`EdgeKind`] came from.
///
/// Inverse of [`crate::classify`]; the two must stay in step.
fn to_attr_of(kind: &EdgeKind) -> &'static str {
    match kind {
        EdgeKind::SceneMember => "objects",
        EdgeKind::AttributeBinding => "geometryattributes",
        EdgeKind::SurfaceShader => "surfaceshader",
        EdgeKind::DisplacementShader => "displacementshader",
        EdgeKind::VolumeShader => "volumeshader",
        EdgeKind::InstanceSource => "sourcemodels",
        EdgeKind::Screen => "screens",
        EdgeKind::OutputLayer => "outputlayers",
        EdgeKind::OutputDriver => "outputdrivers",
        // Handled by the caller, which has the port names.
        EdgeKind::ShaderNetwork { .. } => "",
    }
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
        quoted(&arg.name),
        quoted(&type_name(arg)),
        element_count(arg)
    )?;

    // 3Delight brackets whenever there is more than one scalar, and
    // leaves a lone scalar bare.
    let scalars = scalar_count(arg);
    if scalars > 1 {
        write!(out, "[ ")?;
    }

    match &arg.data {
        OwnedData::F32(v) => write_scalars(out, v)?,
        OwnedData::F64(v) => {
            let formatted: Vec<String> =
                v.iter().copied().map(format_f64).collect();
            write!(out, "{}", formatted.join(" "))?;
        }
        OwnedData::I32(v) => write_scalars(out, v)?,
        OwnedData::I64(v) => write_scalars(out, v)?,
        OwnedData::String(v) => {
            let values: Vec<String> = v.iter().map(|s| quoted(s)).collect();
            write!(out, "{}", values.join(" "))?;
        }
        // Returned above; a `Reference` never reaches here.
        OwnedData::Reference(_) => {}
    }

    if scalars > 1 {
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
    let sized = if arg.array_length > 1 {
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
mod tests {
    use super::{format_f64, quoted};
    use crate::{OwnedArg, OwnedData, Scene, write_stream};
    use nsi_trait::Type;

    /// The values 3Delight writes, captured from its own `apistream`
    /// output. Rust's `Display` disagrees with every one of the first
    /// four, so this is what stops the emitter drifting back.
    #[test]
    fn doubles_format_the_way_3delight_writes_them() {
        for (value, expected) in [
            (0.1f64, "0.10000000000000001"),
            (1.0 / 3.0, "0.33333333333333331"),
            (1e-7, "9.9999999999999995e-08"),
            (1e20, "1e+20"),
            (0.5, "0.5"),
            (1.0, "1"),
            (-0.0, "-0"),
            (0.0, "0"),
        ] {
            assert_eq!(format_f64(value), expected, "for {value}");
        }
    }

    /// A quote closes the literal and a newline ends the statement, so
    /// an unescaped value turns into parseable ɴsɪ. The stream is a
    /// persisted, cross-language format; this is data corruption, not
    /// cosmetics.
    #[test]
    fn a_string_cannot_inject_a_statement() {
        let injected = "say \"hi\"\nCreate \"evil\" \"mesh\"";
        let escaped = quoted(injected);

        assert!(!escaped.contains('\n'), "no raw newline: {escaped}");
        assert_eq!(
            escaped.matches('"').count() - escaped.matches("\\\"").count(),
            2,
            "only the delimiters are unescaped quotes"
        );
    }

    /// The same, end to end: a handle and a value both carrying a quote
    /// and a newline must leave the stream one statement per line.
    #[test]
    fn a_recorded_scene_with_hostile_strings_stays_one_statement_a_line() {
        let mut scene = Scene::default();
        scene.create("me\"ss\ny", "mesh").expect("new handle");
        scene.set_attribute(
            "me\"ss\ny",
            vec![OwnedArg {
                name: "na\"me".to_string(),
                type_tag: Type::String,
                array_length: 1,
                flags: 0,
                data: OwnedData::String(vec![
                    "Create \"evil\" \"mesh\"".to_string(),
                ]),
            }],
        );

        let mut out = Vec::new();
        write_stream(&scene, &mut out).expect("write");
        let text = String::from_utf8(out).expect("utf-8");

        assert_eq!(
            text.lines().filter(|l| l.starts_with("Create ")).count(),
            1,
            "exactly one Create statement:\n{text}"
        );

        // And every quote on every line is either a delimiter or
        // escaped, so a reader tokenises the line we meant to write.
        for line in text.lines() {
            let bare = line.replace("\\\\", "").matches('"').count()
                - line.replace("\\\\", "").matches("\\\"").count();
            assert_eq!(bare % 2, 0, "unbalanced quotes in {line:?}");
            assert!(
                !line.trim_start().starts_with("evil"),
                "a value escaped its literal: {line:?}"
            );
        }
    }
}
