# Data Model: Shading Profile

## Entities

- **Profile** -- a version: the definitive set of NodeDefs and ClosureDefs.
  Owner: `nsi-profile` crate (name tentative, see `plan.md`).
- **ClosureDef** -- name, parameters (types, units, ranges), semantic
  reference (which BSDF/behavior, normalization). E.g. `emission`,
  `diffuse`, `microfacet`.
- **NodeDef** -- name, input/output ports with types and defaults, ᴏsʟ
  reference implementation, GPU emitter. Pattern/utility/material nodes.
- **Network** -- the ɴsɪ-side shader graph: `shader` nodes addressing
  profile NodeDefs via the `nsi-profile:<node>@<version>` scheme (R7),
  connected with standard `NSIConnect`.
- **NetworkModule** -- translation output: SPIR-V module + ParameterBlock
  layout + closure signature (which closures this network can emit).
- **ParameterBlock** -- versioned, documented layout of animatable
  parameters for a translated network (R6). This is a wire format.
- **ValidationReport** -- per-scene: conforming | violations
  (node handle, construct, profile version consulted).

## Profile v1 Tables (Frozen 2026-07-26)

Owner: `crates/nsi-profile` (`src/closure.rs`, `src/v1.rs`,
`src/registry.rs`). The registry in code is the machine-readable copy of
record; this section is the human-readable freeze. Any change to these
tables is a profile version bump (additive ⇒ minor, semantic/layout ⇒
major).

### ClosureDefs (v1)

| Closure | Parameters | Semantic reference |
| --- | --- | --- |
| `diffuse` | `shading_normal` normal, `albedo` color [0,1], `roughness` float [0,1] (σ) | Oren-Nayar; integrates to ≤ albedo. |
| `microfacet` | `shading_normal`, `tangent`, `roughness` [0,1], `anisotropy` [0,1], mode `reflect`\|`refract`, Fresnel `conductor{eta,k color}`\|`dielectric{ior ≥ 1}` | GGX, Smith shadowing. |
| `sheen` | `shading_normal`, `albedo` color, `roughness` float | Retro-sheen fabric lobe. |
| `emission` | weight color = radiance, W·sr⁻¹·m⁻² | ɴsɪ lights are geometry with `emission()`. |
| `transparent` | weight color | Straight-through transmission. |
| `holdout` | -- | Matte/holdout. |

Subsurface is deferred to v2 (spec R2 resolution).

### NodeDefs (v1, 18 nodes)

Pattern/utility: `constant_float`, `constant_color`, `uv`, `image`,
`mix_color`, `math_color` (op enumerant: add/multiply/min/max/…),
`remap_float`, `normal_map`, `mix_bsdf`, `add_bsdf`.

Material: `diffuse_bsdf`, `metal_bsdf` (conductor GGX), `dielectric_bsdf`
(reflect+refract GGX), `sheen_bsdf`, `transparent_bsdf`,
`emission_surface`, `holdout_surface`, `surface` (terminal:
bsdf/emissive/opacity → Surface).

Every NodeDef carries both targets (spec R5): an ᴏsʟ 1.12 reference
(`osl/<node>.osl`) and a GLSL 4.60 GPU source (`glsl/<node>.glsl`), per
the R4 resolution (SPIR-V compilation is a backend step behind the
`GpuEmitter` trait). Port-naming rule: port names double as ɴsɪ attribute
names and as parameter identifiers in both targets, so ᴏsʟ/GLSL keywords
and closure built-in names are avoided (`shading_normal`, `base_color`,
`emissive`, `out_bsdf`, …). The `uv` node has no UV-set index input --
that would require `getattribute()`, which is on the normative exclusion
list; multiple UV sets are deferred.

## Translation Pipeline

```text
NSI shader nodes + connections
  --resolve NodeDefs (version check)--> Network
  --validate (US3)--> ValidationReport
  --translate--> NetworkModule (SPIR-V + ParameterBlock layout)
  --parameter edits only--> ParameterBlock update (no re-translation)
  --topology edits (connect/disconnect)--> re-translate
```

The parameter/topology split mirrors ɴsɪ's own edit model: attribute edits
are cheap and frequent; structural edits are transactions.

## Wire Formats

- `shaderfilename` scheme: `"nsi-profile:<node>@<version>"` on standard
  `shader` nodes; shader parameters remain ordinary ɴsɪ attributes. No new
  node types, no new API calls.
- ParameterBlock layout: std430-compatible, field order = declaration order
  of the NodeDef parameters actually referenced; layout documented per
  profile version and stable within it.
- Profile version: semantic-versioned; a network validated against version
  N must validate against N.x. Unknown versions fail loudly (R1).

## Ownership And Migration

- The profile crate owns NodeDefs, ClosureDefs, ᴏsʟ references, emitters,
  and the validator. Backends consume NetworkModules; offline renderers
  consume the ᴏsʟ references. One owner, two targets (constitution VII).
- Version bumps: additive changes (new nodes/closures) are minor; any
  change to existing semantics, parameters, or ParameterBlock layout is
  major. No silent migration -- the validator names the required version.
