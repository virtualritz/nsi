//! Lua script emission.
//!
//! ɴsɪ has three front ends -- the C API, the stream, and a Lua script
//! -- and this writes the third. The scene comes out as the calls that
//! would rebuild it:
//!
//! ```lua
//! nsi.Create("cam", "perspectivecamera")
//! nsi.SetAttribute("cam", {name="fov", data={45}, type=nsi.TypeFloat})
//! nsi.Connect("cam", "", ".root", "objects")
//! ```
//!
//! # What Lua cannot say
//!
//! ɴsɪ's Lua binding is narrower than its C API, in three ways that all
//! lose data silently, so each is refused instead:
//!
//! - **Types.** There is no `nsi.TypeDouble`, no `nsi.TypeInt64` and no
//!   pointer type. Those names are `nil`, and a parameter table whose
//!   `type` is `nil` is a runtime error in the renderer; passing the
//!   value untyped instead makes a double a `float`, and a 64-bit
//!   integer comes back as a different number entirely.
//! - **Flags.** A parameter table has `name`, `data`, `type` and
//!   `arraylength`, and nothing else. `per_vertex`, `per_face` and
//!   `linear_interpolation` cannot be said, and a per-vertex normal
//!   emitted without its flag rebuilds a different surface.
//! - **Empty string arrays.** Setting one from Lua aborts 3Delight with
//!   a heap error rather than reporting a problem.
//!
//! A wrong answer that renders is worse than a loud one, so
//! [`write_lua`] fails rather than emitting any of them.
//!
//! `doublematrix` is the exception among the wide types --
//! `nsi.TypeDoubleMatrix` exists -- so a transform survives.
//!
//! # Grouping
//!
//! One attribute per `nsi.SetAttribute` call, matching
//! [`crate::write_stream`]. Lua would allow all of a node's attributes
//! in one call, but then the two emitters would disagree about
//! statement boundaries and could not be compared against one another.

use crate::{OwnedArg, OwnedData, Scene};
use core::fmt;
use nsi_ffi_wrap::nsi_sys::NSIParamFlags;
use nsi_trait::Type;
use std::io::{self, Write};

/// Why a scene could not be written as a Lua script.
#[derive(Debug)]
#[non_exhaustive]
pub enum LuaError {
    /// Writing failed.
    Io(io::Error),
    /// ɴsɪ's Lua binding has no name for this attribute's type.
    ///
    /// `f64` and `i64` arguments have no Lua spelling: `nsi.TypeDouble`
    /// and `nsi.TypeInt64` are `nil`, and passing a *typed* parameter
    /// whose type is `nil` is a runtime error in the renderer, while
    /// passing the value untyped silently makes a double a `float` and
    /// a large integer a different number.
    Inexpressible {
        /// The node carrying the attribute.
        handle: String,
        /// The attribute name.
        attribute: String,
        /// The ɴsɪ type that has no Lua spelling.
        type_tag: Type,
    },
    /// The argument carries ɴsɪ flags that its Lua binding cannot
    /// express.
    ///
    /// A Lua parameter table has `name`, `data`, `type` and
    /// `arraylength` -- and nothing for `per_vertex`, `per_face` or
    /// `linear_interpolation`. Emitting the argument without them
    /// rebuilds a *different surface*: a per-vertex normal becomes a
    /// per-varying one, which on a subdivision mesh is a different
    /// shape. Verified against 3Delight: every spelling of a flag key
    /// is ignored.
    InexpressibleFlags {
        /// The node carrying the attribute.
        handle: String,
        /// The attribute name.
        attribute: String,
        /// The flags that have no Lua spelling.
        flags: i32,
    },
    /// An empty string array.
    ///
    /// `{name="x", data={}, type=nsi.TypeString}` aborts 3Delight with
    /// a heap error rather than reporting a problem, so emitting one
    /// hands a consumer a script that kills the renderer. Empty numeric
    /// arrays are fine.
    EmptyStringArray {
        /// The node carrying the attribute.
        handle: String,
        /// The attribute name.
        attribute: String,
    },
}

impl fmt::Display for LuaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::Inexpressible {
                handle,
                attribute,
                type_tag,
            } => write!(
                f,
                "ɴsɪ attribute {attribute:?} on node {handle:?} has type \
                 {type_tag:?}, which ɴsɪ's Lua binding cannot express; \
                 emitting it untyped would change its value"
            ),
            Self::InexpressibleFlags {
                handle,
                attribute,
                flags,
            } => write!(
                f,
                "ɴsɪ attribute {attribute:?} on node {handle:?} carries \
                 flags {flags:#x}, which a Lua parameter table cannot \
                 express; emitting it without them would rebuild a \
                 different surface"
            ),
            Self::EmptyStringArray { handle, attribute } => write!(
                f,
                "ɴsɪ attribute {attribute:?} on node {handle:?} is an \
                 empty string array, which aborts the renderer when a \
                 Lua script sets one"
            ),
        }
    }
}

impl core::error::Error for LuaError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Inexpressible { .. }
            | Self::InexpressibleFlags { .. }
            | Self::EmptyStringArray { .. } => None,
        }
    }
}

impl From<io::Error> for LuaError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Write `scene` as a Lua script.
///
/// # Errors
///
/// [`LuaError::Inexpressible`] for an attribute whose ɴsɪ type has no
/// Lua spelling, and [`LuaError::Io`] for a write failure.
pub fn write_lua<W: Write>(scene: &Scene, out: &mut W) -> Result<(), LuaError> {
    for (handle, node) in scene.nodes() {
        // The reserved handles need no `create`, exactly as in a stream.
        if !crate::is_reserved(handle) {
            writeln!(
                out,
                "nsi.Create({}, {})",
                quoted_str(handle),
                quoted_str(&node.node_type)
            )?;
        }

        for arg in node.attrs.values() {
            write!(out, "nsi.SetAttribute({}, ", quoted_str(handle))?;
            write_arg(out, handle, arg)?;
            writeln!(out, ")")?;
        }

        // In call order, as `write_stream` explains.
        for calls in node.samples.values() {
            for (time, arg) in calls {
                write!(
                    out,
                    "nsi.SetAttributeAtTime({}, {}, ",
                    quoted_str(handle),
                    lua_number(*time)
                )?;
                write_arg(out, handle, arg)?;
                writeln!(out, ")")?;
            }
        }
    }

    for edge in scene.edges() {
        let (from_port, to_port) = match &edge.kind {
            crate::EdgeKind::ShaderNetwork { from_port, to_port } => {
                (from_port.as_str(), to_port.as_str())
            }
            other => ("", other.to_attr()),
        };

        write!(
            out,
            "nsi.Connect({}, {}, {}, {}",
            quoted_str(&edge.from),
            quoted_str(from_port),
            quoted_str(&edge.to),
            quoted_str(to_port)
        )?;
        for arg in &edge.args {
            write!(out, ", ")?;
            write_arg(out, &edge.from, arg)?;
        }
        writeln!(out, ")")?;
    }

    Ok(())
}

/// One parameter table: `{name=..., data=..., type=...}`.
fn write_arg<W: Write>(
    out: &mut W,
    handle: &str,
    arg: &OwnedArg,
) -> Result<(), LuaError> {
    // A Lua parameter table has room for `name`, `data`, `type` and
    // `arraylength` -- and nothing else. Dropping a flag silently
    // rebuilds a different surface.
    let inexpressible = NSIParamFlags::PerFace.bits()
        | NSIParamFlags::PerVertex.bits()
        | NSIParamFlags::InterpolateLinear.bits();
    if arg.flags & inexpressible != 0 {
        return Err(LuaError::InexpressibleFlags {
            handle: handle.to_string(),
            attribute: arg.name.clone(),
            flags: arg.flags,
        });
    }

    // An empty string array aborts the renderer outright.
    if matches!(&arg.data, OwnedData::String(values) if values.is_empty()) {
        return Err(LuaError::EmptyStringArray {
            handle: handle.to_string(),
            attribute: arg.name.clone(),
        });
    }

    let type_name =
        lua_type_name(arg.type_tag).ok_or_else(|| LuaError::Inexpressible {
            handle: handle.to_string(),
            attribute: arg.name.clone(),
            type_tag: arg.type_tag,
        })?;

    write!(out, "{{name={}", quoted_str(&arg.name))?;
    if arg.flags & NSIParamFlags::IsArray.bits() != 0 {
        write!(out, ", arraylength={}", arg.array_length)?;
    }

    // A typed parameter's data must be a table even when it holds one
    // value: the renderer reads a bare number as an empty array and
    // writes `"name" "float" 0 [ ]`.
    write!(out, ", data={{")?;
    match &arg.data {
        OwnedData::F32(values) => write_numbers(out, values)?,
        OwnedData::F64(values) => write_numbers(out, values)?,
        OwnedData::I32(values) => write_numbers(out, values)?,
        OwnedData::I64(values) => write_numbers(out, values)?,
        OwnedData::String(values) => {
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.write_all(b", ")?;
                }
                out.write_all(&self::quoted(value))?;
            }
        }
        OwnedData::Reference(_) => {
            // Unreachable: `lua_type_name` rejected it above.
        }
    }
    write!(out, "}}, type=nsi.{type_name}}}")?;

    Ok(())
}

fn write_numbers<W: Write, T: LuaNumber>(
    out: &mut W,
    values: &[T],
) -> io::Result<()> {
    let rendered: Vec<String> = values.iter().map(LuaNumber::to_lua).collect();
    write!(out, "{}", rendered.join(", "))
}

/// A value Lua writes as a number literal.
trait LuaNumber {
    fn to_lua(&self) -> String;
}

impl LuaNumber for f32 {
    fn to_lua(&self) -> String {
        lua_number(f64::from(*self))
    }
}

impl LuaNumber for f64 {
    fn to_lua(&self) -> String {
        lua_number(*self)
    }
}

impl LuaNumber for i32 {
    fn to_lua(&self) -> String {
        self.to_string()
    }
}

impl LuaNumber for i64 {
    fn to_lua(&self) -> String {
        self.to_string()
    }
}

/// A Lua number literal.
///
/// Lua numbers are doubles, so this uses Rust's shortest round-tripping
/// form rather than the stream's `%.17g`: the consumer is a Lua parser,
/// not 3Delight's stream reader.
fn lua_number(value: f64) -> String {
    if value.is_finite() {
        let rendered = value.to_string();
        // Lua needs a decimal point to read a float as a float.
        if rendered.contains(['.', 'e', 'E']) {
            rendered
        } else {
            format!("{rendered}.0")
        }
    } else if value.is_nan() {
        "0/0".to_string()
    } else if value.is_sign_negative() {
        "-math.huge".to_string()
    } else {
        "math.huge".to_string()
    }
}

/// One Lua string literal, escaped.
fn quoted(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 2);
    out.push(b'"');
    for &byte in value {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
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
    // Unreachable, and stated rather than repaired: `quoted` substitutes
    // only for `"`, `\`, `\n`, `\t` and bytes below `0x20`, all ASCII,
    // and in valid UTF-8 an ASCII byte is always a whole code point --
    // lead and continuation bytes are `0x80` and above. So every
    // multi-byte sequence is copied through contiguously and unchanged.
    // Falling back to a lossy conversion here would quietly rewrite an
    // identifier, which is the one thing this crate refuses to do.
    String::from_utf8(quoted(value.as_bytes()))
        .expect("escaping only ASCII preserves UTF-8")
}

/// The `nsi.Type*` name for an ɴsɪ type, where Lua has one.
///
/// `None` for the three ɴsɪ types with no Lua spelling. Verified against
/// 3Delight 2.9.207: naming `nsi.TypeDouble` or `nsi.TypeInt64` is a
/// parse error in the renderer.
const fn lua_type_name(type_tag: Type) -> Option<&'static str> {
    match type_tag {
        Type::F32 => Some("TypeFloat"),
        Type::I32 => Some("TypeInteger"),
        Type::String => Some("TypeString"),
        Type::Color => Some("TypeColor"),
        Type::Point => Some("TypePoint"),
        Type::Vector => Some("TypeVector"),
        Type::Normal => Some("TypeNormal"),
        Type::MatrixF32 => Some("TypeMatrix"),
        Type::MatrixF64 => Some("TypeDoubleMatrix"),
        Type::F64 | Type::I64 | Type::Reference | Type::Invalid => None,
    }
}
