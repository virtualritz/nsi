//! Node definitions -- ports, types, defaults and the two implementations of
//! record.
//!
//! Profile nodes are ɴsɪ-native definitions (requirement R3, resolved
//! 2026-07-26): the node table is *derived from* a MaterialX standard-library
//! subset but owned here, so that no backend inherits a MaterialX or glslang
//! dependency (constitution principle VII, one owner).
//!
//! Every [`NodeDef`] carries **both** implementations required by R5 and by
//! the `nodedef_completeness` contract row:
//!
//! - [`NodeDef::osl_source`] -- an ᴏsʟ 1.12 reference implementation, the
//!   thing an offline renderer (3Delight) actually executes.
//! - [`NodeDef::glsl_source`] -- a GLSL 4.60 function, the GPU source of
//!   record (requirement R4, resolved 2026-07-26). Compilation to SPIR-V is
//!   a backend step behind [`GpuEmitter`](crate::emit::GpuEmitter); this
//!   crate takes no compiler-toolchain dependency.
use core::fmt;

/// The data type of a port.
///
/// [`PortType::Bsdf`] and [`PortType::Surface`] are closure types: they
/// carry a closure value, not a number, and are therefore never
/// parameter-block fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PortType {
    /// Scalar `float`.
    Float,
    /// Signed 32-bit `int`.
    Int,
    /// Linear RGB triple.
    Color,
    /// Direction triple; transforms as a vector.
    Vector,
    /// Direction triple; transforms as a normal (inverse transpose).
    Normal,
    /// Position triple; transforms as a point.
    Point,
    /// String. Restricted to enum-like constants and resource names --
    /// string *operations* are outside the profile (see the crate-level
    /// exclusion list), and string ports are never animatable.
    String,
    /// A scattering closure.
    Bsdf,
    /// A complete surface closure: scattering plus emission, opacity and
    /// holdout. This is the type of a network terminal.
    Surface,
}

impl PortType {
    /// Returns whether this is a closure type ([`Bsdf`](Self::Bsdf) or
    /// [`Surface`](Self::Surface)).
    #[must_use]
    pub const fn is_closure(self) -> bool {
        matches!(self, Self::Bsdf | Self::Surface)
    }

    /// Returns whether values of this type can live in a
    /// [`ParameterBlock`](crate::parameter_block) -- i.e. whether they are
    /// numeric and therefore animatable without re-translation.
    #[must_use]
    pub const fn is_block_eligible(self) -> bool {
        matches!(
            self,
            Self::Float
                | Self::Int
                | Self::Color
                | Self::Vector
                | Self::Normal
                | Self::Point
        )
    }

    /// The `std430` base alignment in bytes, or `None` for non-block types.
    ///
    /// Three-component types have a base alignment of 16 (`4N`) and a size
    /// of 12 -- see [`parameter_block`](crate::parameter_block).
    #[must_use]
    pub const fn std430_align(self) -> Option<usize> {
        match self {
            Self::Float | Self::Int => Some(4),
            Self::Color | Self::Vector | Self::Normal | Self::Point => Some(16),
            Self::String | Self::Bsdf | Self::Surface => None,
        }
    }

    /// The `std430` size in bytes, or `None` for non-block types.
    #[must_use]
    pub const fn std430_size(self) -> Option<usize> {
        match self {
            Self::Float | Self::Int => Some(4),
            Self::Color | Self::Vector | Self::Normal | Self::Point => Some(12),
            Self::String | Self::Bsdf | Self::Surface => None,
        }
    }

    /// The GLSL 4.60 spelling of this type.
    ///
    /// String ports are passed to GLSL as `int` enumerants resolved at
    /// translation time; closures use the aggregates from `glsl/common.glsl`.
    #[must_use]
    pub const fn glsl_type(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Int | Self::String => "int",
            Self::Color | Self::Vector | Self::Normal | Self::Point => "vec3",
            Self::Bsdf => "NsiClosure",
            Self::Surface => "NsiSurface",
        }
    }

    /// The ᴏsʟ spelling of this type.
    #[must_use]
    pub const fn osl_type(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Int => "int",
            Self::Color => "color",
            Self::Vector => "vector",
            Self::Normal => "normal",
            Self::Point => "point",
            Self::String => "string",
            Self::Bsdf | Self::Surface => "closure color",
        }
    }
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Float => "float",
            Self::Int => "int",
            Self::Color => "color",
            Self::Vector => "vector",
            Self::Normal => "normal",
            Self::Point => "point",
            Self::String => "string",
            Self::Bsdf => "bsdf",
            Self::Surface => "surface",
        };
        f.write_str(name)
    }
}

/// A shading global a port can default to.
///
/// Ports defaulting to a global read geometry, not an animatable parameter,
/// so they are excluded from the parameter block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Global {
    /// Shading normal.
    N,
    /// Geometric normal.
    Ng,
    /// Shading position.
    P,
    /// Surface tangent (`dPdu`).
    U,
    /// Primary texture coordinate, as `(u, v, 0)`.
    Uv,
}

impl Global {
    /// The ᴏsʟ global expression this maps to.
    #[must_use]
    pub const fn osl_expr(self) -> &'static str {
        match self {
            Self::N => "N",
            Self::Ng => "Ng",
            Self::P => "P",
            Self::U => "normalize(dPdu)",
            Self::Uv => "vector(u, v, 0)",
        }
    }

    /// The GLSL shading-context member this maps to.
    #[must_use]
    pub const fn glsl_expr(self) -> &'static str {
        match self {
            Self::N => "ctx.N",
            Self::Ng => "ctx.Ng",
            Self::P => "ctx.P",
            Self::U => "ctx.U",
            Self::Uv => "ctx.uv",
        }
    }
}

/// The default value of a port.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortDefault {
    /// A `float` literal.
    Float(f32),
    /// An `int` literal.
    Int(i32),
    /// A triple literal -- used for color, vector, normal and point ports.
    Triple([f32; 3]),
    /// A string literal. String ports are not animatable.
    String(&'static str),
    /// A shading global. Not animatable.
    Global(Global),
}

impl PortDefault {
    /// Returns whether this default makes its port a parameter-block field.
    ///
    /// Literal numeric defaults do; strings and globals do not.
    #[must_use]
    pub const fn is_block_eligible(self) -> bool {
        matches!(self, Self::Float(_) | Self::Int(_) | Self::Triple(_))
    }
}

/// An input or output port of a [`NodeDef`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Port {
    /// Port name. Also the ɴsɪ attribute name on the `shader` node and the
    /// parameter name in both reference implementations.
    pub name: &'static str,
    /// Data type.
    pub ty: PortType,
    /// Default value. Outputs and closure inputs have none.
    pub default: Option<PortDefault>,
    /// For [`PortType::String`] ports: the permitted constants. Empty means
    /// "any string" (resource names such as a texture file).
    pub allowed: &'static [&'static str],
    /// What the port means.
    pub doc: &'static str,
}

impl Port {
    /// Returns whether this port becomes a parameter-block field when left
    /// unconnected.
    #[must_use]
    pub fn is_block_eligible(&self) -> bool {
        self.ty.is_block_eligible()
            && self.default.is_some_and(PortDefault::is_block_eligible)
    }
}

/// A node in the profile vocabulary, with both implementations of record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeDef {
    /// Node name, as it appears in `nsi-profile:<node>@<version>`.
    pub name: &'static str,
    /// What the node computes.
    pub description: &'static str,
    /// Input ports, in declaration order. This order is also the
    /// parameter-block field order for this node.
    pub inputs: &'static [Port],
    /// Output ports, in declaration order.
    pub outputs: &'static [Port],
    /// The closures this node can emit, by
    /// [`ClosureDef::name`](crate::closure::ClosureDef::name). Contributes
    /// to a translated network's closure signature.
    pub closures: &'static [&'static str],
    /// The ᴏsʟ 1.12 reference implementation (R5).
    pub osl_source: &'static str,
    /// The GLSL 4.60 function body -- the GPU source of record (R4).
    pub glsl_source: &'static str,
}

impl NodeDef {
    /// Looks an input port up by name.
    #[must_use]
    pub fn input(&self, name: &str) -> Option<&'static Port> {
        self.inputs.iter().find(|port| port.name == name)
    }

    /// Looks an output port up by name.
    #[must_use]
    pub fn output(&self, name: &str) -> Option<&'static Port> {
        self.outputs.iter().find(|port| port.name == name)
    }

    /// The single output port, which every v1 node has exactly one of.
    ///
    /// # Panics
    ///
    /// Panics if the node has no outputs. Every v1 node has one; the
    /// `nodedef_completeness` contract row enforces it.
    #[must_use]
    pub fn sole_output(&self) -> &'static Port {
        self.outputs
            .first()
            .expect("every profile node declares at least one output")
    }
}
