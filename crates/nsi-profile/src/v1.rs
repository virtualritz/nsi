//! The profile v1 node table.
//!
//! Eighteen ɴsɪ-native nodes derived from a MaterialX standard-library
//! subset (requirement R3, resolved 2026-07-26). The full table with port
//! lists is reproduced in the crate-level documentation.
//!
//! # Conventions
//!
//! - Node names are lowercase snake case; they are what appears after
//!   `nsi-profile:` in a `shaderfilename`.
//! - Every node has exactly one output, named after its type: `out_float`,
//!   `out_color`, `out_vector`, `out_normal`, `out_bsdf`, `out_surface`.
//! - Port names are also the ɴsɪ attribute names on the `shader` node *and*
//!   the parameter names in both reference implementations, so a scene sets
//!   them the same way for either target.
//! - Port names avoid ᴏsʟ and GLSL keywords: the normal input is
//!   `shading_normal`, not `normal`; the terminal's emission input is
//!   `emissive`, not `emission`. The ᴏsʟ shader *identifier* is prefixed
//!   `nsi_` for the same reason, while the file name is the node name.
use crate::node::{Global, NodeDef, Port, PortDefault, PortType};

/// An input port with a literal or global default.
const fn input(
    name: &'static str,
    ty: PortType,
    default: PortDefault,
    doc: &'static str,
) -> Port {
    Port {
        name,
        ty,
        default: Some(default),
        allowed: &[],
        doc,
    }
}

/// A `String` input port restricted to a fixed set of constants.
const fn enumerant(
    name: &'static str,
    default: &'static str,
    allowed: &'static [&'static str],
    doc: &'static str,
) -> Port {
    Port {
        name,
        ty: PortType::String,
        default: Some(PortDefault::String(default)),
        allowed,
        doc,
    }
}

/// A closure input port. Closure ports have no default; left unconnected
/// they are the null closure.
const fn closure_input(
    name: &'static str,
    ty: PortType,
    doc: &'static str,
) -> Port {
    Port {
        name,
        ty,
        default: None,
        allowed: &[],
        doc,
    }
}

/// An output port.
const fn output(name: &'static str, ty: PortType, doc: &'static str) -> Port {
    Port {
        name,
        ty,
        default: None,
        allowed: &[],
        doc,
    }
}

/// The `shading_normal` input, shared by every node that builds a shading
/// frame.
const SHADING_NORMAL: Port = input(
    "shading_normal",
    PortType::Normal,
    PortDefault::Global(Global::N),
    "Shading normal; defaults to the geometric shading normal.",
);

/// The `tangent` input, shared by every anisotropic node.
const TANGENT: Port = input(
    "tangent",
    PortType::Vector,
    PortDefault::Global(Global::U),
    "Anisotropy tangent; defaults to the surface tangent.",
);

/// A uniform `float`.
pub const CONSTANT_FLOAT: NodeDef = NodeDef {
    name: "constant_float",
    description: "A uniform float value.",
    inputs: &[input(
        "value",
        PortType::Float,
        PortDefault::Float(0.0),
        "The value to output.",
    )],
    outputs: &[output("out_float", PortType::Float, "The value.")],
    closures: &[],
    osl_source: include_str!("../osl/constant_float.osl"),
    glsl_source: include_str!("../glsl/constant_float.glsl"),
};

/// A uniform linear-RGB color.
pub const CONSTANT_COLOR: NodeDef = NodeDef {
    name: "constant_color",
    description: "A uniform color value.",
    inputs: &[input(
        "value",
        PortType::Color,
        PortDefault::Triple([0.5, 0.5, 0.5]),
        "The value to output, linear RGB.",
    )],
    outputs: &[output("out_color", PortType::Color, "The value.")],
    closures: &[],
    osl_source: include_str!("../osl/constant_color.osl"),
    glsl_source: include_str!("../glsl/constant_color.glsl"),
};

/// The primary texture coordinate.
pub const UV: NodeDef = NodeDef {
    name: "uv",
    description: "The primary texture coordinate, as (u, v, 0).",
    inputs: &[],
    outputs: &[output(
        "out_vector",
        PortType::Vector,
        "The texture coordinate.",
    )],
    closures: &[],
    osl_source: include_str!("../osl/uv.osl"),
    glsl_source: include_str!("../glsl/uv.glsl"),
};

/// A texture file sample.
pub const IMAGE: NodeDef = NodeDef {
    name: "image",
    description: "Samples a texture file at a texture coordinate.",
    inputs: &[
        input(
            "filename",
            PortType::String,
            PortDefault::String(""),
            "Texture file to sample. Changing it re-translates the network.",
        ),
        input(
            "uv",
            PortType::Vector,
            PortDefault::Global(Global::Uv),
            "Texture coordinate; defaults to the primary UV set.",
        ),
        input(
            "default_color",
            PortType::Color,
            PortDefault::Triple([0.0, 0.0, 0.0]),
            "Returned where the texture is missing.",
        ),
    ],
    outputs: &[output("out_color", PortType::Color, "The sampled color.")],
    closures: &[],
    osl_source: include_str!("../osl/image.osl"),
    glsl_source: include_str!("../glsl/image.glsl"),
};

/// Linear interpolation between two colors.
pub const MIX_COLOR: NodeDef = NodeDef {
    name: "mix_color",
    description: "Linear interpolation between two colors.",
    inputs: &[
        input(
            "a",
            PortType::Color,
            PortDefault::Triple([0.0, 0.0, 0.0]),
            "Value at t = 0.",
        ),
        input(
            "b",
            PortType::Color,
            PortDefault::Triple([1.0, 1.0, 1.0]),
            "Value at t = 1.",
        ),
        input(
            "t",
            PortType::Float,
            PortDefault::Float(0.5),
            "Interpolant, clamped to [0, 1].",
        ),
    ],
    outputs: &[output(
        "out_color",
        PortType::Color,
        "The interpolated color.",
    )],
    closures: &[],
    osl_source: include_str!("../osl/mix_color.osl"),
    glsl_source: include_str!("../glsl/mix_color.glsl"),
};

/// Componentwise binary math on two colors.
///
/// Folds what would otherwise be `add_color`, `multiply_color` and
/// `clamp_color` into one node with an enumerant, which keeps the v1 table
/// smaller without losing expressiveness: clamping is `min` followed by
/// `max`.
pub const MATH_COLOR: NodeDef = NodeDef {
    name: "math_color",
    description: "Componentwise binary math on two colors.",
    inputs: &[
        input(
            "a",
            PortType::Color,
            PortDefault::Triple([0.0, 0.0, 0.0]),
            "Left operand.",
        ),
        input(
            "b",
            PortType::Color,
            PortDefault::Triple([0.0, 0.0, 0.0]),
            "Right operand.",
        ),
        enumerant(
            "op",
            "add",
            &["add", "subtract", "multiply", "divide", "min", "max"],
            "The operation to apply. Division by zero yields zero.",
        ),
    ],
    outputs: &[output("out_color", PortType::Color, "The result.")],
    closures: &[],
    osl_source: include_str!("../osl/math_color.osl"),
    glsl_source: include_str!("../glsl/math_color.glsl"),
};

/// Affine remap of a float from one range to another.
pub const REMAP_FLOAT: NodeDef = NodeDef {
    name: "remap_float",
    description: "Affine remap of a float from one range to another.",
    inputs: &[
        input(
            "in_float",
            PortType::Float,
            PortDefault::Float(0.0),
            "Value to remap.",
        ),
        input(
            "inlow",
            PortType::Float,
            PortDefault::Float(0.0),
            "Low end of the input range.",
        ),
        input(
            "inhigh",
            PortType::Float,
            PortDefault::Float(1.0),
            "High end of the input range.",
        ),
        input(
            "outlow",
            PortType::Float,
            PortDefault::Float(0.0),
            "Low end of the output range.",
        ),
        input(
            "outhigh",
            PortType::Float,
            PortDefault::Float(1.0),
            "High end of the output range.",
        ),
    ],
    outputs: &[output("out_float", PortType::Float, "The remapped value.")],
    closures: &[],
    osl_source: include_str!("../osl/remap_float.osl"),
    glsl_source: include_str!("../glsl/remap_float.glsl"),
};

/// Tangent-space normal map decoding.
pub const NORMAL_MAP: NodeDef = NodeDef {
    name: "normal_map",
    description: "Decodes a tangent space normal map.",
    inputs: &[
        input(
            "in_color",
            PortType::Color,
            PortDefault::Triple([0.5, 0.5, 1.0]),
            "Encoded normal, [0, 1] per channel.",
        ),
        input(
            "strength",
            PortType::Float,
            PortDefault::Float(1.0),
            "Scales the tangential deflection.",
        ),
        SHADING_NORMAL,
        TANGENT,
    ],
    outputs: &[output(
        "out_normal",
        PortType::Normal,
        "The decoded world-space normal.",
    )],
    closures: &[],
    osl_source: include_str!("../osl/normal_map.osl"),
    glsl_source: include_str!("../glsl/normal_map.glsl"),
};

/// Linear interpolation between two scattering closures.
pub const MIX_BSDF: NodeDef = NodeDef {
    name: "mix_bsdf",
    description: "Linear interpolation between two scattering closures.",
    inputs: &[
        closure_input("a", PortType::Bsdf, "Closure at t = 0."),
        closure_input("b", PortType::Bsdf, "Closure at t = 1."),
        input(
            "t",
            PortType::Float,
            PortDefault::Float(0.5),
            "Interpolant, clamped to [0, 1].",
        ),
    ],
    outputs: &[output("out_bsdf", PortType::Bsdf, "The blended closure.")],
    closures: &[],
    osl_source: include_str!("../osl/mix_bsdf.osl"),
    glsl_source: include_str!("../glsl/mix_bsdf.glsl"),
};

/// Sum of two scattering closures.
pub const ADD_BSDF: NodeDef = NodeDef {
    name: "add_bsdf",
    description: "Sum of two scattering closures; not renormalised.",
    inputs: &[
        closure_input("a", PortType::Bsdf, "First closure."),
        closure_input("b", PortType::Bsdf, "Second closure."),
    ],
    outputs: &[output("out_bsdf", PortType::Bsdf, "The sum.")],
    closures: &[],
    osl_source: include_str!("../osl/add_bsdf.osl"),
    glsl_source: include_str!("../glsl/add_bsdf.glsl"),
};

/// Oren-Nayar diffuse reflection.
pub const DIFFUSE_BSDF: NodeDef = NodeDef {
    name: "diffuse_bsdf",
    description: "Oren-Nayar diffuse reflection.",
    inputs: &[
        input(
            "base_color",
            PortType::Color,
            PortDefault::Triple([0.18, 0.18, 0.18]),
            "Diffuse albedo, [0, 1] per channel.",
        ),
        input(
            "roughness",
            PortType::Float,
            PortDefault::Float(0.0),
            "Oren-Nayar sigma, [0, 1]; 0 is Lambert.",
        ),
        SHADING_NORMAL,
    ],
    outputs: &[output("out_bsdf", PortType::Bsdf, "The diffuse closure.")],
    closures: &["diffuse"],
    osl_source: include_str!("../osl/diffuse_bsdf.osl"),
    glsl_source: include_str!("../glsl/diffuse_bsdf.glsl"),
};

/// GGX conductor with artistic reflectivity parameters.
pub const METAL_BSDF: NodeDef = NodeDef {
    name: "metal_bsdf",
    description: "GGX conductor with artistic reflectivity parameters.",
    inputs: &[
        input(
            "base_color",
            PortType::Color,
            PortDefault::Triple([0.95, 0.93, 0.88]),
            "Reflectivity at normal incidence, [0, 1).",
        ),
        input(
            "edge_color",
            PortType::Color,
            PortDefault::Triple([1.0, 1.0, 1.0]),
            "Reflectivity at grazing incidence, [0, 1].",
        ),
        input(
            "roughness",
            PortType::Float,
            PortDefault::Float(0.2),
            "Perceptual roughness, [0, 1]; alpha is its square.",
        ),
        input(
            "anisotropy",
            PortType::Float,
            PortDefault::Float(0.0),
            "Tangential roughness stretch, [0, 1].",
        ),
        SHADING_NORMAL,
        TANGENT,
    ],
    outputs: &[output("out_bsdf", PortType::Bsdf, "The conductor closure.")],
    closures: &["microfacet"],
    osl_source: include_str!("../osl/metal_bsdf.osl"),
    glsl_source: include_str!("../glsl/metal_bsdf.glsl"),
};

/// GGX dielectric with reflection and refraction lobes.
pub const DIELECTRIC_BSDF: NodeDef = NodeDef {
    name: "dielectric_bsdf",
    description: "GGX dielectric with reflection and refraction lobes.",
    inputs: &[
        input(
            "ior",
            PortType::Float,
            PortDefault::Float(1.5),
            "Refractive index, >= 1.",
        ),
        input(
            "roughness",
            PortType::Float,
            PortDefault::Float(0.0),
            "Perceptual roughness, [0, 1]; alpha is its square.",
        ),
        input(
            "transmission_color",
            PortType::Color,
            PortDefault::Triple([1.0, 1.0, 1.0]),
            "Tint of the refracted lobe, [0, 1] per channel.",
        ),
        SHADING_NORMAL,
        TANGENT,
    ],
    outputs: &[output(
        "out_bsdf",
        PortType::Bsdf,
        "The dielectric closure.",
    )],
    closures: &["microfacet"],
    osl_source: include_str!("../osl/dielectric_bsdf.osl"),
    glsl_source: include_str!("../glsl/dielectric_bsdf.glsl"),
};

/// Microflake sheen.
pub const SHEEN_BSDF: NodeDef = NodeDef {
    name: "sheen_bsdf",
    description: "Microflake sheen for fabric-like rim scattering.",
    inputs: &[
        input(
            "base_color",
            PortType::Color,
            PortDefault::Triple([1.0, 1.0, 1.0]),
            "Sheen albedo, [0, 1] per channel.",
        ),
        input(
            "roughness",
            PortType::Float,
            PortDefault::Float(0.3),
            "Rim width, [0, 1].",
        ),
        SHADING_NORMAL,
    ],
    outputs: &[output("out_bsdf", PortType::Bsdf, "The sheen closure.")],
    closures: &["sheen"],
    osl_source: include_str!("../osl/sheen_bsdf.osl"),
    glsl_source: include_str!("../glsl/sheen_bsdf.glsl"),
};

/// Straight-through transmission.
pub const TRANSPARENT_BSDF: NodeDef = NodeDef {
    name: "transparent_bsdf",
    description: "Straight-through transmission.",
    inputs: &[input(
        "base_color",
        PortType::Color,
        PortDefault::Triple([1.0, 1.0, 1.0]),
        "Transmission tint, [0, 1] per channel.",
    )],
    outputs: &[output(
        "out_bsdf",
        PortType::Bsdf,
        "The transparent closure.",
    )],
    closures: &["transparent"],
    osl_source: include_str!("../osl/transparent_bsdf.osl"),
    glsl_source: include_str!("../glsl/transparent_bsdf.glsl"),
};

/// Lambertian emitter -- the ɴsɪ light.
pub const EMISSION_SURFACE: NodeDef = NodeDef {
    name: "emission_surface",
    description: "Lambertian emitter; the profile's only light construct.",
    inputs: &[
        input(
            "base_color",
            PortType::Color,
            PortDefault::Triple([1.0, 1.0, 1.0]),
            "Emission color, linear RGB.",
        ),
        input(
            "intensity",
            PortType::Float,
            PortDefault::Float(1.0),
            "Radiance multiplier, W/sr/m^2.",
        ),
    ],
    outputs: &[output(
        "out_surface",
        PortType::Surface,
        "The emissive surface.",
    )],
    closures: &["emission"],
    osl_source: include_str!("../osl/emission_surface.osl"),
    glsl_source: include_str!("../glsl/emission_surface.glsl"),
};

/// Holdout / matte surface.
pub const HOLDOUT_SURFACE: NodeDef = NodeDef {
    name: "holdout_surface",
    description: "Holdout/matte surface.",
    inputs: &[],
    outputs: &[output(
        "out_surface",
        PortType::Surface,
        "The holdout surface.",
    )],
    closures: &["holdout"],
    osl_source: include_str!("../osl/holdout_surface.osl"),
    glsl_source: include_str!("../glsl/holdout_surface.glsl"),
};

/// The network terminal.
pub const SURFACE: NodeDef = NodeDef {
    name: "surface",
    description: "Network terminal: scattering, emission and opacity.",
    inputs: &[
        closure_input("bsdf", PortType::Bsdf, "Scattering closure."),
        closure_input(
            "emissive",
            PortType::Surface,
            "Emissive surface closure.",
        ),
        input(
            "opacity",
            PortType::Color,
            PortDefault::Triple([1.0, 1.0, 1.0]),
            "Coverage, [0, 1] per channel; below one adds `transparent`.",
        ),
    ],
    outputs: &[output(
        "out_surface",
        PortType::Surface,
        "The composed surface.",
    )],
    closures: &["transparent"],
    osl_source: include_str!("../osl/surface.osl"),
    glsl_source: include_str!("../glsl/surface.glsl"),
};

/// The shared GLSL preamble every assembled network module starts with.
pub const GLSL_COMMON: &str = include_str!("../glsl/common.glsl");

/// The profile v1 node table, in canonical order.
///
/// The order is the order node functions appear in an assembled module; it
/// is part of the translation determinism guarantee.
pub const V1_NODES: &[NodeDef] = &[
    CONSTANT_FLOAT,
    CONSTANT_COLOR,
    UV,
    IMAGE,
    MIX_COLOR,
    MATH_COLOR,
    REMAP_FLOAT,
    NORMAL_MAP,
    MIX_BSDF,
    ADD_BSDF,
    DIFFUSE_BSDF,
    METAL_BSDF,
    DIELECTRIC_BSDF,
    SHEEN_BSDF,
    TRANSPARENT_BSDF,
    EMISSION_SURFACE,
    HOLDOUT_SURFACE,
    SURFACE,
];
