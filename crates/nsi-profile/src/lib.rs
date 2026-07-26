//! The ɴsɪ shading profile -- a fixed, versioned closure and node
//! vocabulary that a realtime backend can evaluate by *translating* shader
//! networks to portable GPU code, instead of executing arbitrary ᴏsʟ.
//!
//! This implements the direction decided in
//! `specs/001-gpu-pixel-streaming/research.md` D7: ᴏsʟ's only GPU backend is
//! the NVIDIA/OptiX path, and the hard part of writing another is not
//! codegen but the runtime contract -- `trace()`, `getattribute()`, ustrings
//! as renderer callbacks. Network translation against a fixed vocabulary is
//! cross-vendor today, and it matches how ɴsɪ is actually used: shader nodes
//! form networks, and ɴsɪ lights *are* emissive shaders, so the translated
//! vocabulary must include `emission`.
//!
//! Closures stay the boundary between shading and the integrator, exactly as
//! in ᴏsʟ. What the profile fixes is *which* nodes and closures exist per
//! version.
//!
//! # No New ɴsɪ Surface
//!
//! A conforming scene needs no new node types and no new API calls. A
//! profile node is a standard `shader` node whose `shaderfilename` attribute
//! reads
//!
//! ```text
//! nsi-profile:<node>@<version>
//! ```
//!
//! -- for example `nsi-profile:diffuse_bsdf@1` or `nsi-profile:image@1.0`.
//! Shader parameters are ordinary attributes and the graph is built with
//! ordinary connections. See the [`version`] module for the grammar, the
//! compatibility rules, and the typed failures.
//!
//! # Pipeline
//!
//! ```text
//! shader nodes + connections
//!   --resolve (Registry)-------> Network
//!   --validate (US3)-----------> ValidationReport
//!   --translate----------------> NetworkModule (GLSL + ParameterBlock)
//!   --GpuEmitter---------------> EmittedShader (GLSL, SPIR-V, WGSL, ...)
//! ```
//!
//! Parameter edits patch the [`ParameterBlock`](parameter_block); topology
//! edits re-translate. [`edit::classify`] decides which.
//!
//! ```
//! use nsi_profile::{
//!     network::{Connection, Network, ParamValue, ShaderNode},
//!     registry::Registry,
//!     translate::translate,
//!     validate::validate,
//!     version::PROFILE_V1,
//! };
//!
//! let registry = Registry::v1();
//!
//! let network = Network::new(
//!     vec![
//!         ShaderNode::new("mat", "nsi-profile:diffuse_bsdf@1")
//!             .with_param("roughness", ParamValue::Float(0.3)),
//!         ShaderNode::new("out", "nsi-profile:surface@1"),
//!     ],
//!     vec![Connection::new("mat", "out_bsdf", "out", "bsdf")],
//! );
//!
//! assert!(validate(&network, &registry, &PROFILE_V1).is_conforming());
//!
//! let module = translate(&network, &registry).unwrap();
//! assert_eq!(module.closure_signature(), ["diffuse", "transparent"]);
//! ```
//!
//! # Profile v1 Closures
//!
//! Frozen by requirement R2. Parameters, units, ranges and normalization are
//! documented per closure in [`closure`].
//!
//! | Closure | Model | ᴏsʟ mapping |
//! | --- | --- | --- |
//! | `diffuse` | Oren-Nayar | `oren_nayar_diffuse_bsdf` |
//! | `microfacet` | GGX, reflect + refract, conductor or dielectric Fresnel | `conductor_bsdf` / `dielectric_bsdf` |
//! | `sheen` | microflake sheen | `sheen_bsdf` |
//! | `emission` | Lambertian emitter; the weight *is* radiance | `emission()` |
//! | `transparent` | straight-through transmission | `transparent()` |
//! | `holdout` | matte/holdout | `holdout()` |
//!
//! Subsurface is deferred to v2: its GPU evaluation strategy (diffusion vs.
//! random walk) would dictate integrator architecture, which is a separate
//! coverage item.
//!
//! # Profile v1 Nodes
//!
//! Eighteen ɴsɪ-native nodes derived from a MaterialX standard-library
//! subset (requirement R3). Every node ships **both** an ᴏsʟ 1.12 reference
//! implementation (`osl/<node>.osl`) and a GLSL 4.60 function
//! (`glsl/<node>.glsl`); see [`node`] and [`v1`].
//!
//! Pattern and utility nodes:
//!
//! | Node | Inputs | Output |
//! | --- | --- | --- |
//! | `constant_float` | `value` | `out_float` |
//! | `constant_color` | `value` | `out_color` |
//! | `uv` | -- | `out_vector` |
//! | `image` | `filename`, `uv`, `default_color` | `out_color` |
//! | `mix_color` | `a`, `b`, `t` | `out_color` |
//! | `math_color` | `a`, `b`, `op` | `out_color` |
//! | `remap_float` | `in_float`, `inlow`, `inhigh`, `outlow`, `outhigh` | `out_float` |
//! | `normal_map` | `in_color`, `strength`, `shading_normal`, `tangent` | `out_normal` |
//! | `mix_bsdf` | `a`, `b`, `t` | `out_bsdf` |
//! | `add_bsdf` | `a`, `b` | `out_bsdf` |
//!
//! Material nodes:
//!
//! | Node | Inputs | Output | Closures |
//! | --- | --- | --- | --- |
//! | `diffuse_bsdf` | `base_color`, `roughness`, `shading_normal` | `out_bsdf` | `diffuse` |
//! | `metal_bsdf` | `base_color`, `edge_color`, `roughness`, `anisotropy`, `shading_normal`, `tangent` | `out_bsdf` | `microfacet` |
//! | `dielectric_bsdf` | `ior`, `roughness`, `transmission_color`, `shading_normal`, `tangent` | `out_bsdf` | `microfacet` |
//! | `sheen_bsdf` | `base_color`, `roughness`, `shading_normal` | `out_bsdf` | `sheen` |
//! | `transparent_bsdf` | `base_color` | `out_bsdf` | `transparent` |
//! | `emission_surface` | `base_color`, `intensity` | `out_surface` | `emission` |
//! | `holdout_surface` | -- | `out_surface` | `holdout` |
//! | `surface` | `bsdf`, `emissive`, `opacity` | `out_surface` | `transparent` |
//!
//! `math_color` folds what would otherwise be `add_color`,
//! `multiply_color` and `clamp_color` into one node with an `op` enumerant;
//! clamping is `min` followed by `max`. Port names avoid ᴏsʟ and GLSL
//! keywords -- hence `shading_normal` rather than `normal`, and `emissive`
//! rather than `emission` -- because a port name is simultaneously the ɴsɪ
//! attribute name and the parameter name in both reference implementations.
//!
//! # Versioning Policy
//!
//! Profile versions are semantic versions. [`PROFILE_V1`] is `1.0.0`.
//!
//! - **Minor** bump: additive only -- new nodes, new closures, new optional
//!   ports. A network that validates against `N.m` still validates against
//!   `N.m+1`.
//! - **Major** bump: any change to existing semantics, port sets, closure
//!   parameters, or to the [`ParameterBlock`](parameter_block) layout
//!   algorithm.
//! - **No silent migration.** An unknown node or an unsatisfiable version is
//!   a typed [`ResolveError`] naming what was requested and what is
//!   available; the validator names the version it consulted on every
//!   violation.
//!
//! Both the `shaderfilename` scheme and the ParameterBlock layout are wire
//! formats in the sense of the project constitution, and are documented with
//! their compatibility and failure modes in [`version`] and
//! [`parameter_block`].
//!
//! # Normative Exclusions
//!
//! The following are **outside profile v1**. The validator rejects them; it
//! never silently strips them. Adding any of them requires a profile version
//! bump.
//!
//! - `trace()` and every other ray-casting call from shading.
//! - `getattribute()` against arbitrary scene state.
//! - String operations -- concatenation, formatting, pattern matching,
//!   substring extraction. Comparing a *uniform enumerant* parameter inside
//!   a reference implementation is not a network construct and is
//!   unaffected.
//! - Dictionary lookups (`dict_find`, `dict_value`).
//! - Arbitrary hand-written ᴏsʟ, which is what a `shaderfilename` not using
//!   the `nsi-profile:` scheme means. It remains fully supported for offline
//!   renderers through the same ɴsɪ scene -- it simply is not translatable.
//!
//! Texture sampling *is* in the profile: it is a renderer service with a
//! bounded contract, unlike the callbacks above.
//!
//! # What Is Not Here Yet
//!
//! MaterialX interchange (US5) is a separate, feature-gated concern and is
//! not implemented; the node table is *derived from* a MaterialX subset, but
//! nothing in this crate depends on MaterialX. The offline/realtime parity
//! harness (US2, US4) needs a running 3Delight and lives with the repo's
//! image-comparison machinery.
#![deny(missing_docs)]

pub mod closure;
pub mod edit;
pub mod emit;
pub mod error;
pub mod network;
pub mod node;
pub mod parameter_block;
pub mod registry;
pub mod translate;
pub mod v1;
pub mod validate;
pub mod version;

pub use closure::{ClosureDef, ClosureParam};
pub use edit::{Edit, EditClass, classify};
pub use emit::{EmittedShader, GlslPassthroughEmitter, GpuEmitter};
pub use error::{Error, ResolveError};
pub use network::{Connection, Network, ParamValue, ShaderNode};
pub use node::{Global, NodeDef, Port, PortDefault, PortType};
pub use parameter_block::{Field, ParameterBlockLayout};
pub use registry::{Profile, Registry, Resolved};
pub use translate::{NetworkModule, TextureBinding, translate};
pub use validate::{ValidationReport, Violation, validate};
pub use version::{PROFILE_V1, RequestedVersion, SchemeRef, Version};
