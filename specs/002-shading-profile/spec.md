# Feature Spec: Shading Profile (Closure/Node Vocabulary)

The fixed, versioned vocabulary of shader nodes and closures that a realtime
ɴsɪ backend evaluates by *translating shader networks* to portable GPU code
(SPIR-V), instead of executing arbitrary ᴏsʟ. Direction decided in
`specs/001-gpu-pixel-streaming/research.md` D7. Closures stay the boundary
between shading and the integrator, exactly as in ᴏsʟ; what the profile
fixes is *which* nodes and closures exist per version.

## User Stories

### User Story 1: Versioned Closure Vocabulary (P1)

As a realtime-backend author, I want a fixed, versioned set of closures
with defined parameter semantics, so that my integrator can evaluate
materials without an ᴏsʟ runtime.

**Acceptance Criteria**

- Given a profile version, when a backend declares support for it, then
  every closure in that version has defined parameters, units, and
  normalization documented in the profile.
- Given a network using only version-N vocabulary, when validated against
  version N, then validation passes; when it uses anything else, validation
  fails naming the offending node/closure.

### User Story 2: Offline/Realtime Parity (P1)

As a look developer, I want every profile node to ship an ᴏsʟ reference
implementation, so that the same ɴsɪ scene renders with matching material
appearance offline (3Delight executing the ᴏsʟ references) and realtime
(GPU code translated from the same network).

**Acceptance Criteria**

- Given a profile test scene, when rendered via the ᴏsʟ references and via
  the GPU translation, then per-material outputs match within the declared
  tolerance (image-comparison harness, not pixel-exact).
- Given a parameter change on a profile node, when re-rendered both ways,
  then both outputs change consistently.

### User Story 3: Loud Profile Validation (P2)

As a pipeline TD, I want a validation tool that reports whether a scene's
shader networks are inside a profile version, so that unsupported ᴏsʟ fails
loudly before a realtime render, not silently mid-frame.

**Acceptance Criteria**

- Given a scene with an out-of-profile shader, when validated, then the
  report names the shader node handle, the construct, and the profile
  version consulted.
- Given a conforming scene, when validated, then the report is empty and
  exits zero.

### User Story 4: Emission Parity (P2)

As a scene author, I want lights -- which in ɴsɪ are geometry with
`emission()` closures -- fully covered by the profile, so that area, spot,
point, directional, and HDR environment lighting work identically offline
and realtime.

**Acceptance Criteria**

- Given each documented ɴsɪ light construction pattern, when rendered both
  ways, then illumination matches within tolerance.

### User Story 5: MaterialX Interchange (P3)

As a DCC integrator, I want profile networks to map to/from MaterialX
documents, so that existing MaterialX assets flow into ɴsɪ scenes without
re-authoring.

**Acceptance Criteria**

- Given a MaterialX document using the mapped node set, when imported, then
  the resulting ɴsɪ shader network validates against the profile and
  renders equivalently.

## Non-Goals

- Executing arbitrary ᴏsʟ on the GPU (rejected as option C in D7).
- A new authoring language -- authoring is node networks (and MaterialX
  interchange); hand-written ᴏsʟ remains first-class for offline renderers
  through the same scene.
- Pixel-exact parity with 3Delight -- parity is material appearance within
  declared tolerance; sampling/integration differences are out of scope.
- `trace()`, `getattribute()` against arbitrary scene state, string
  operations, and dictionary lookups -- excluded from profile v1 (these are
  the runtime-callback constructs that make full ᴏsʟ-on-Vulkan hard).
- The realtime backend/integrator itself (coverage-order item 5).

## Requirements

- R1: The profile is versioned; networks declare (or are validated against)
  a profile version. Version negotiation failures are typed and loud.
- R2: The closure set is fixed per version. RESOLVED (2026-07-26): the v1
  closure list is the proposed baseline -- `diffuse` (Oren-Nayar),
  `microfacet` GGX (reflect and refract, with conductor and dielectric
  Fresnel parameterizations), `sheen`, `emission`, `transparent`,
  `holdout`. Subsurface is deferred to v2: it is the one closure whose GPU
  evaluation strategy (diffusion vs. random-walk) would dictate integrator
  architecture, which is coverage-order item 5, not this feature.
- R3: The node set is derived from a MaterialX standard-library subset,
  with ɴsɪ-native naming. RESOLVED (2026-07-26): profile nodes are
  ɴsɪ-native definitions with a MaterialX mapping layer (feature-gated
  import, US5). Reusing MaterialX nodedefs/shadergen was rejected because
  it drags the glslang GLSL-then-SPIR-V toolchain into the core path (see
  Risks) and makes MaterialX a mandatory dependency of every backend;
  ɴsɪ-native definitions keep one owner (constitution VII) and make the
  MaterialX subset an interchange concern, not a foundation.
- R4: Translation target is SPIR-V. RESOLVED (2026-07-26): SPIR-V
  passthrough is sufficient for v1 -- the realtime backend is Vulkan-first
  (001 resolution) and `wgpu` accepts SPIR-V passthrough on Vulkan. No WGSL
  emitter in v1. Codegen stays behind a trait (`GpuEmitter`) so a WGSL or
  native emitter can be added without touching NodeDefs: in v1 each NodeDef
  carries a GLSL 4.60 function body as its GPU source of record, and the
  translator assembles the network module source + ParameterBlock layout;
  compilation of the assembled module to SPIR-V (via glslang/shaderc) is a
  backend build step behind the same trait, so the profile crate itself
  takes no compiler toolchain dependency.
- R5: Every profile node ships an ᴏsʟ reference implementation; the
  reference set is what offline renderers execute (US2 parity).
- R6: Each translated network yields a deterministic parameter-block layout
  (a wire format -- documented and versioned like `stream.*`).
- R7: Profile nodes are addressed from ɴsɪ `shader` nodes via a
  distinguished `shaderfilename` scheme (working sketch:
  `"nsi-profile:<node>@<version>"`), so a conforming scene needs no new
  node types (mirrors R1 of feature 001).
- R8: A standalone validator implements US3 and is usable in CI.

## Risks

- MaterialX shadergen has no WGSL target and its Vulkan story is
  GLSL-then-SPIR-V; if R3 lands on MaterialX nodedefs, the SPIR-V path
  inherits that toolchain. Mitigation: R4 clarification; keep codegen
  behind a trait so glslang-based and native emitters are swappable.
- Closure semantics drift between the ᴏsʟ references (as 3Delight evaluates
  them) and GPU code. Mitigation: US2's image-comparison harness is the
  contract, run in CI against fixture scenes; the repo already has
  expected-image test machinery (`RUST_TEST_UPDATE`).
- Scope creep toward full ᴏsʟ. Mitigation: the excluded-constructs list in
  Non-Goals is normative; additions require a profile version bump.
- Tolerance definition disputes. Mitigation: per-fixture thresholds
  declared in the contract, not ad hoc.
