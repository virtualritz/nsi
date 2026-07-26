//! The ParameterBlock -- a versioned wire format (requirement R6).
//!
//! A translated network yields a byte layout for exactly those parameters an
//! engine may animate without re-translating. This is the materials
//! counterpart of ɴsɪ's own edit model: attribute edits are cheap and
//! frequent, structural edits are transactions (`research.md` D4).
//!
//! # Layout Rules
//!
//! The layout is `std430`-compatible, so the same bytes can be uploaded to a
//! GLSL `layout(std430) buffer` block without repacking.
//!
//! | Port type | Base alignment | Size |
//! | --- | --- | --- |
//! | `float` | 4 | 4 |
//! | `int` | 4 | 4 |
//! | `color`, `vector`, `normal`, `point` (`vec3`) | 16 | 12 |
//!
//! Three-component types follow the `std430` rule that a three-component
//! vector has the base alignment of a four-component one (16 bytes) while
//! occupying only 12 -- so a `float` may follow a `vec3` immediately, at
//! `offset + 12`.
//!
//! Each field is placed at the next offset that satisfies its base
//! alignment; the total size is rounded up to 16 bytes, the block's own base
//! alignment. Scalars are little-endian, matching every SPIR-V target this
//! profile addresses.
//!
//! # Field Order
//!
//! Deterministic, and derived only from the *shape* of the network, never
//! from which parameters a scene happened to set:
//!
//! 1. Nodes in topological order (ties broken by declaration order, see
//!    [`Network::topological_order`](crate::network::Network::topological_order)).
//! 2. Within a node, the [`NodeDef`](crate::node::NodeDef) input ports in
//!    declaration order.
//! 3. A port contributes a field iff it is numeric, has a literal default,
//!    and is *not* fed by a connection. Connected ports get their value from
//!    the upstream node; string ports and ports defaulting to a shading
//!    global are not animatable and are baked into the module.
//!
//! Because presence does not depend on whether the scene set the parameter,
//! an engine can animate a parameter it never authored.
//!
//! # Compatibility And Versioning
//!
//! The layout algorithm is part of the profile version. Any change to it --
//! alignment, ordering, or which ports qualify -- is a **major** version
//! bump, because previously recorded buffers would otherwise be
//! misinterpreted. Additive vocabulary changes do not disturb the layout of
//! an existing network: a network's layout depends only on the nodes it
//! actually uses.
//!
//! # Failure Modes
//!
//! [`ParameterBlockLayout::write_param`] never guesses. Writing an unknown
//! or non-block parameter is [`Error::NotABlockParameter`] -- the caller
//! must re-translate; a wrong value type is
//! [`Error::ParameterTypeMismatch`]; a short buffer is
//! [`Error::BufferTooSmall`].
use core::fmt;

use crate::{error::Error, network::ParamValue, node::PortType};

/// One animatable parameter in the block.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    /// Handle of the `shader` node the parameter belongs to.
    pub node_handle: String,
    /// The parameter (port) name.
    pub param: String,
    /// The parameter type.
    pub ty: PortType,
    /// Byte offset from the start of the block.
    pub offset: usize,
    /// Size in bytes -- 4 for scalars, 12 for triples.
    pub size: usize,
}

/// The byte layout of a translated network's animatable parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ParameterBlockLayout {
    fields: Vec<Field>,
    total_size: usize,
}

impl ParameterBlockLayout {
    /// Computes the layout for a sequence of `(handle, param, type)` entries
    /// already in field order.
    ///
    /// Entries whose type is not block-eligible are skipped rather than
    /// misplaced; callers filter them out beforehand, so this is a
    /// belt-and-braces guard, not a fallback.
    #[must_use]
    pub fn new(
        entries: impl IntoIterator<Item = (String, String, PortType)>,
    ) -> Self {
        let mut cursor = 0_usize;

        let fields: Vec<Field> = entries
            .into_iter()
            .filter_map(|(node_handle, param, ty)| {
                ty.std430_align()
                    .zip(ty.std430_size())
                    .map(|(align, size)| {
                        let offset = cursor.next_multiple_of(align);
                        cursor = offset + size;

                        Field {
                            node_handle,
                            param,
                            ty,
                            offset,
                            size,
                        }
                    })
            })
            .collect();

        Self {
            fields,
            total_size: cursor.next_multiple_of(16),
        }
    }

    /// The field for `handle.param`, if the block carries it.
    #[must_use]
    pub fn field(&self, handle: &str, param: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|field| field.node_handle == handle && field.param == param)
    }

    /// All fields, in layout order.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// The block size in bytes, rounded up to the block's own alignment.
    #[must_use]
    pub const fn total_size(&self) -> usize {
        self.total_size
    }

    /// A zeroed buffer of exactly the block's size.
    #[must_use]
    pub fn zeroed_buffer(&self) -> Vec<u8> {
        vec![0; self.total_size]
    }

    /// Patches one parameter into a block-sized byte buffer.
    ///
    /// # Errors
    ///
    /// - [`Error::BufferTooSmall`] if `buffer` is shorter than
    ///   [`total_size`](Self::total_size).
    /// - [`Error::NotABlockParameter`] if the block does not carry
    ///   `handle.param` -- the network must be re-translated instead.
    /// - [`Error::ParameterTypeMismatch`] if `value` does not have the
    ///   field's type.
    pub fn write_param(
        &self,
        buffer: &mut [u8],
        handle: &str,
        param: &str,
        value: &ParamValue,
    ) -> Result<(), Error> {
        if buffer.len() < self.total_size {
            Err(Error::BufferTooSmall {
                needed: self.total_size,
                got: buffer.len(),
            })
        } else {
            let field = self.field(handle, param).ok_or_else(|| {
                Error::NotABlockParameter {
                    handle: handle.to_string(),
                    param: param.to_string(),
                }
            })?;

            if field.ty != value.port_type() {
                Err(Error::ParameterTypeMismatch {
                    handle: handle.to_string(),
                    param: param.to_string(),
                    expected: field.ty,
                    found: value.port_type(),
                })
            } else {
                let bytes = match value {
                    ParamValue::Float(scalar) => scalar.to_le_bytes().to_vec(),
                    ParamValue::Int(scalar) => scalar.to_le_bytes().to_vec(),
                    ParamValue::Color(triple)
                    | ParamValue::Vector(triple)
                    | ParamValue::Normal(triple)
                    | ParamValue::Point(triple) => triple
                        .iter()
                        .flat_map(|scalar| scalar.to_le_bytes())
                        .collect(),
                    ParamValue::String(_) => Vec::new(),
                };

                buffer[field.offset..field.offset + field.size]
                    .copy_from_slice(&bytes);

                Ok(())
            }
        }
    }
}

impl fmt::Display for ParameterBlockLayout {
    /// The stable text form used by the `parameter_block_layout` golden-file
    /// test. Columns are `offset size type handle.param`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "# nsi-profile ParameterBlock, std430, little-endian")?;
        writeln!(f, "# offset size type handle.param")?;

        self.fields.iter().try_for_each(|field| {
            writeln!(
                f,
                "{} {} {} {}.{}",
                field.offset,
                field.size,
                field.ty,
                field.node_handle,
                field.param
            )
        })?;

        write!(f, "total_size {}", self.total_size)
    }
}
