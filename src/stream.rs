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
use nsi_trait::Type;
use std::io::{self, Write};

/// Write `scene` as an ɴsɪ stream.
pub fn write_stream<W: Write>(scene: &Scene, out: &mut W) -> io::Result<()> {
    for (handle, node) in &scene.nodes {
        writeln!(out, r#"Create "{}" "{}""#, handle, node.node_type)?;

        for arg in node.attrs.values() {
            writeln!(out, r#"SetAttribute "{handle}""#)?;
            write_arg(out, arg)?;
        }

        for (time, attrs) in &node.time_attrs {
            for arg in attrs.values() {
                writeln!(out, r#"SetAttributeAtTime "{handle}" {time}"#)?;
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
            r#"Connect "{}" "{}" "{}" "{}""#,
            edge.from, from_port, edge.to, to_port
        )?;
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
    write!(
        out,
        r#"  "{}" "{}" {} "#,
        arg.name,
        type_name(arg),
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
        OwnedData::F64(v) => write_scalars(out, v)?,
        OwnedData::I32(v) => write_scalars(out, v)?,
        OwnedData::I64(v) => write_scalars(out, v)?,
        OwnedData::String(v) => {
            let quoted: Vec<String> = v.iter().map(|s| format!(r#""{s}""#)).collect();
            write!(out, "{}", quoted.join(" "))?;
        }
        // Host pointers have no stream representation; 3Delight omits
        // them from apistream output too.
        OwnedData::Reference(_) => {}
    }

    if scalars > 1 {
        write!(out, " ]")?;
    }
    writeln!(out)
}

fn write_scalars<W: Write, T: std::fmt::Display>(out: &mut W, values: &[T]) -> io::Result<()> {
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
    if arg.array_length > 1 {
        format!("{base}[{}]", arg.array_length)
    } else {
        base.to_string()
    }
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
