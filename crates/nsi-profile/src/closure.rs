//! The closure vocabulary -- the shading/integrator boundary.
//!
//! Closures are the only interface between a shading network and the
//! integrator, exactly as in ᴏsʟ (`research.md` D2): a network computes
//! closure weights and parameters, the integrator owns sampling. What the
//! profile fixes is *which* closures exist in a given version.
//!
//! # Profile v1 Closures
//!
//! Frozen by requirement R2 (resolved 2026-07-26).
//!
//! | Closure | Model | ᴏsʟ mapping |
//! | --- | --- | --- |
//! | [`diffuse`](DIFFUSE) | Oren-Nayar | `oren_nayar_diffuse_bsdf` |
//! | [`microfacet`](MICROFACET) | GGX, reflect + refract, conductor or dielectric Fresnel | `conductor_bsdf` / `dielectric_bsdf` |
//! | [`sheen`](SHEEN) | microflake sheen | `sheen_bsdf` |
//! | [`emission`](EMISSION) | Lambertian emitter | `emission()` |
//! | [`transparent`](TRANSPARENT) | straight-through transmission | `transparent()` |
//! | [`holdout`](HOLDOUT) | matte/holdout | `holdout()` |
//!
//! **Subsurface is deferred to v2.** It is the one closure whose GPU
//! evaluation strategy (diffusion vs. random walk) would dictate integrator
//! architecture, which is a separate coverage item, not this feature.
//!
//! # Energy Normalization
//!
//! Every v1 scattering closure is energy-conserving in the same sense: the
//! directional-hemispherical reflectance integrates to **at most** its
//! `albedo` (or, for [`microfacet`](MICROFACET), to at most the Fresnel
//! reflectance), never above it. Weights applied to a closure by the network
//! (via [`mix_bsdf`](crate::v1::MIX_BSDF) and
//! [`add_bsdf`](crate::v1::ADD_BSDF)) are the caller's responsibility: the
//! profile does not renormalize sums, so `add_bsdf` can be pushed above
//! unity deliberately.
//!
//! [`emission`](EMISSION) is not normalized -- its weight *is* radiance, in
//! W·sr⁻¹·m⁻². ɴsɪ has no light nodes; lights are geometry carrying an
//! emissive surface, so this closure is part of *every* profile version
//! (contract invariant).
use crate::node::PortType;

/// One parameter of a [`ClosureDef`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClosureParam {
    /// Parameter name, matching the ᴏsʟ closure argument where one exists.
    pub name: &'static str,
    /// Data type.
    pub ty: PortType,
    /// Physical units, or `"dimensionless"`.
    pub units: &'static str,
    /// Valid range, as a human-readable interval.
    pub range: &'static str,
    /// What the parameter defaults to, and what the default means.
    pub default_desc: &'static str,
}

/// A closure in the profile vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClosureDef {
    /// Closure name as used in a [`NodeDef`](crate::node::NodeDef)'s closure
    /// signature.
    pub name: &'static str,
    /// Parameters, in declaration order.
    pub params: &'static [ClosureParam],
    /// The BSDF/behavior this closure references, plus its normalization.
    pub semantic: &'static str,
    /// The ᴏsʟ built-in the reference implementations emit for it (R5).
    pub osl_mapping: &'static str,
}

impl ClosureDef {
    /// Looks a parameter up by name.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&'static ClosureParam> {
        self.params.iter().find(|param| param.name == name)
    }
}

/// Oren-Nayar diffuse reflection.
///
/// Semantics: rough-Lambertian reflection with the Oren-Nayar
/// microfacet-of-Lambertian model; `roughness` is the σ of the Gaussian
/// slope distribution, so `roughness = 0` degenerates to Lambert.
/// Normalization: integrates to at most `albedo` over the hemisphere.
pub const DIFFUSE: ClosureDef = ClosureDef {
    name: "diffuse",
    params: &[
        ClosureParam {
            name: "N",
            ty: PortType::Normal,
            units: "unit vector, world space",
            range: "normalized",
            default_desc: "the shading normal `N`",
        },
        ClosureParam {
            name: "albedo",
            ty: PortType::Color,
            units: "dimensionless reflectance",
            range: "[0, 1] per channel",
            default_desc: "0.18 -- middle grey",
        },
        ClosureParam {
            name: "roughness",
            ty: PortType::Float,
            units: "dimensionless (Oren-Nayar σ)",
            range: "[0, 1]",
            default_desc: "0.0 -- pure Lambert",
        },
    ],
    semantic: "Oren-Nayar diffuse BSDF. Directional-hemispherical reflectance \
               integrates to at most `albedo`; σ = 0 reduces to Lambert.",
    osl_mapping: "oren_nayar_diffuse_bsdf(N, albedo, roughness)",
};

/// GGX microfacet reflection and refraction.
///
/// Semantics: Trowbridge-Reitz (GGX) normal distribution with the matching
/// Smith height-correlated masking-shadowing term. `mode` selects the lobe
/// (`"reflect"` or `"refract"`); `fresnel` selects the Fresnel
/// parameterization -- `"conductor"` uses complex IOR (`eta`, `k`),
/// `"dielectric"` uses a real `ior`. Anisotropy stretches the roughness
/// along the tangent `U`: `alpha_u = roughness² / (1 - anisotropy·0.9)`,
/// `alpha_v = roughness² · (1 - anisotropy·0.9)`.
///
/// Normalization: with the height-correlated Smith term the single-scatter
/// lobe integrates to at most the Fresnel reflectance; multiple scattering
/// between microfacets is *not* compensated in v1, so very rough conductors
/// darken -- a known, documented deviation shared by both targets, which is
/// what keeps them in parity.
pub const MICROFACET: ClosureDef = ClosureDef {
    name: "microfacet",
    params: &[
        ClosureParam {
            name: "N",
            ty: PortType::Normal,
            units: "unit vector, world space",
            range: "normalized",
            default_desc: "the shading normal `N`",
        },
        ClosureParam {
            name: "U",
            ty: PortType::Vector,
            units: "unit vector, world space",
            range: "normalized, orthogonal to `N`",
            default_desc: "the surface tangent `dPdu`",
        },
        ClosureParam {
            name: "roughness",
            ty: PortType::Float,
            units: "dimensionless, perceptual",
            range: "[0, 1]",
            default_desc: "0.2 -- α = roughness²",
        },
        ClosureParam {
            name: "anisotropy",
            ty: PortType::Float,
            units: "dimensionless",
            range: "[0, 1]",
            default_desc: "0.0 -- isotropic",
        },
        ClosureParam {
            name: "mode",
            ty: PortType::String,
            units: "enumerant",
            range: "`reflect` | `refract`",
            default_desc: "`reflect`",
        },
        ClosureParam {
            name: "fresnel",
            ty: PortType::String,
            units: "enumerant",
            range: "`conductor` | `dielectric`",
            default_desc: "`dielectric`",
        },
        ClosureParam {
            name: "eta",
            ty: PortType::Color,
            units: "refractive index, per channel (conductor only)",
            range: "> 0",
            default_desc: "(0.18, 0.42, 1.37) -- aluminium",
        },
        ClosureParam {
            name: "k",
            ty: PortType::Color,
            units: "extinction coefficient, per channel (conductor only)",
            range: ">= 0",
            default_desc: "(3.42, 2.35, 1.77) -- aluminium",
        },
        ClosureParam {
            name: "ior",
            ty: PortType::Float,
            units: "refractive index (dielectric only)",
            range: ">= 1",
            default_desc: "1.5 -- crown glass",
        },
    ],
    semantic: "GGX (Trowbridge-Reitz) microfacet BSDF with height-correlated \
               Smith masking-shadowing. Single-scatter lobe integrates to at \
               most the Fresnel reflectance; no multiple-scattering \
               compensation in v1.",
    osl_mapping: "conductor_bsdf(N, U, ax, ay, eta, k, \"ggx\") for \
                  `fresnel = conductor`, dielectric_bsdf(N, U, tint, tint, \
                  ax, ay, ior, \"ggx\") for `fresnel = dielectric`",
};

/// Microflake sheen -- retroreflective fabric-like rim scattering.
///
/// Semantics: a normalized microflake sheen lobe layered *over* whatever it
/// is added to; `roughness` widens the rim. Normalization: integrates to at
/// most `albedo`.
pub const SHEEN: ClosureDef = ClosureDef {
    name: "sheen",
    params: &[
        ClosureParam {
            name: "N",
            ty: PortType::Normal,
            units: "unit vector, world space",
            range: "normalized",
            default_desc: "the shading normal `N`",
        },
        ClosureParam {
            name: "albedo",
            ty: PortType::Color,
            units: "dimensionless reflectance",
            range: "[0, 1] per channel",
            default_desc: "1.0 -- white sheen",
        },
        ClosureParam {
            name: "roughness",
            ty: PortType::Float,
            units: "dimensionless",
            range: "[0, 1]",
            default_desc: "0.3",
        },
    ],
    semantic: "Microflake sheen BSDF. Directional-hemispherical reflectance \
               integrates to at most `albedo`.",
    osl_mapping: "sheen_bsdf(N, albedo, roughness)",
};

/// Emitted radiance.
///
/// Semantics: a Lambertian emitter. The closure *weight* is the emitted
/// radiance -- there is no separate parameter -- in W·sr⁻¹·m⁻², uniform over
/// the hemisphere around the geometric normal. ɴsɪ lights are geometry with
/// an emissive surface, so every light pattern (area, spot, point,
/// directional, environment) is expressed through this closure plus geometry
/// and, where applicable, a shaped intensity pattern computed by the
/// network.
///
/// Normalization: none. Emission is a source term, not a reflectance.
pub const EMISSION: ClosureDef = ClosureDef {
    name: "emission",
    params: &[ClosureParam {
        name: "radiance",
        ty: PortType::Color,
        units: "W·sr⁻¹·m⁻²",
        range: ">= 0",
        default_desc: "the closure weight itself; 0 emits nothing",
    }],
    semantic: "Lambertian emitter. The closure weight is emitted radiance in \
               W·sr⁻¹·m⁻²; not normalized.",
    osl_mapping: "radiance * emission()",
};

/// Straight-through transmission.
///
/// Semantics: a Dirac transmission lobe along the incident direction, with
/// no refraction and no absorption of its own. Used for cutouts and for the
/// non-opaque part of [`surface`](crate::v1::SURFACE).
///
/// Normalization: transmits exactly its weight.
pub const TRANSPARENT: ClosureDef = ClosureDef {
    name: "transparent",
    params: &[],
    semantic: "Dirac straight-through transmission. Transmits exactly its \
               weight; no refraction, no absorption.",
    osl_mapping: "transparent()",
};

/// Holdout / matte.
///
/// Semantics: removes the surface from the beauty result and punches the
/// corresponding alpha, so that live-action plates can be composited behind
/// it. Carries no radiance of its own.
///
/// Normalization: not a reflectance; the weight is the holdout fraction in
/// [0, 1].
pub const HOLDOUT: ClosureDef = ClosureDef {
    name: "holdout",
    params: &[],
    semantic: "Holdout/matte. The weight is the holdout fraction in [0, 1]; \
               contributes no radiance.",
    osl_mapping: "holdout()",
};

/// The profile v1 closure table, in canonical order.
pub const V1_CLOSURES: &[ClosureDef] =
    &[DIFFUSE, MICROFACET, SHEEN, EMISSION, TRANSPARENT, HOLDOUT];
