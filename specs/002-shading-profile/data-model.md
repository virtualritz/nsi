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
