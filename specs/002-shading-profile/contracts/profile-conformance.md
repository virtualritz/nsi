# Contract: Profile Conformance And Parity

## Scope

This contract covers profile versioning, validation, translation outputs,
and offline/realtime parity. It does not cover integrator behavior
(sampling, MIS, denoising) or the pixel-streaming transport (feature 001).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| `nsi-profile:<node>@<version>` resolution on standard `shader` nodes; unknown node/version fails loudly | Covered | `crates/nsi-profile/src/version.rs` (`SchemeRef::parse`, `Profile::resolve`), typed `ResolveError` | `cargo test -p nsi-profile resolve_scheme` -- ok (2026-07-26) | `cargo test -p nsi-profile resolve_scheme`. |
| Validator reports out-of-profile constructs with node handle + construct + version (US3) | Covered | `crates/nsi-profile/src/validate.rs` (`validate`, `Violation{node_handle, construct, version_consulted}`) | `cargo test -p nsi-profile validator_violations` -- ok (2026-07-26) | `cargo test -p nsi-profile validator_violations`; CI run on fixture scenes. |
| Conforming fixture scenes validate clean | Covered | `crates/nsi-profile/src/validate.rs`; fixture in `crates/nsi-profile/tests/conformance.rs` | `cargo test -p nsi-profile validator_clean` -- ok (2026-07-26) | `cargo test -p nsi-profile validator_clean`. |
| Every v1 NodeDef has both an ᴏsʟ reference and a GPU emitter | Covered | `crates/nsi-profile/src/registry.rs` + `src/v1.rs` (`osl_source`/`glsl_source` per NodeDef, `osl/`, `glsl/`) | `cargo test -p nsi-profile nodedef_completeness` -- ok (2026-07-26); note: `oslc` compile pass still outstanding (no oslc on CI box) | `cargo test -p nsi-profile nodedef_completeness` (walks the registry). |
| Per-node parity: ᴏsʟ reference vs GPU translation within declared per-fixture tolerance (US2) | Open | None | None | Image-comparison harness vs 3Delight renders of fixture scenes (`DELIGHT` set); thresholds declared per fixture. |
| Emission parity for each documented ɴsɪ light pattern (US4) | Open | None | None | Same harness, light-rig fixtures (area, spot, point, directional, HDR environment). |
| ParameterBlock layout deterministic and stable within a profile version (R6) | Covered | `crates/nsi-profile/src/parameter_block.rs` (std430 layout, documented as versioned wire format) | `cargo test -p nsi-profile parameter_block_layout` -- ok, golden file `tests/golden/parameter_block_v1.txt` (2026-07-26) | `cargo test -p nsi-profile parameter_block_layout` golden-file test. |
| Parameter edits update ParameterBlocks without re-translation; topology edits re-translate | Covered | `crates/nsi-profile/src/edit.rs` (`classify`: `ParameterUpdate{offset,size}` vs `Retranslate`) | `cargo test -p nsi-profile edit_classification` -- ok (2026-07-26) | `cargo test -p nsi-profile edit_classification`. |
| MaterialX import maps the documented subset and validates (US5) | Open | None | None | `cargo test -p nsi-profile --features materialx mtlx_roundtrip` on sample documents. |

## Invariants

- No new ɴsɪ API calls or node types; the profile rides on `shader` nodes,
  attributes, and connections.
- Closures are the only shading/integrator interface; `emission` is part of
  every profile version (ɴsɪ has no light nodes).
- The excluded-construct list (`spec.md` Non-Goals) is normative; the
  validator rejects, never silently strips.
- Parity tolerances are declared per fixture in-repo, not chosen at test
  time.

## Failure Modes

- Unknown profile node/version → typed resolution error naming the handle.
- Out-of-profile network reaching the translator → translator refuses
  (validation is not optional in the pipeline).
- Parity regression beyond tolerance → CI failure listing fixture, node,
  and metric; expected-image updates require explicit human approval
  (constitution, review gates).

## Required Evidence Before Marking Complete

- Source evidence must cite `crates/nsi-profile/src/` symbols (registry,
  validator, translator, emitters) per row.
- Executable evidence: exact `cargo test -p nsi-profile <name>` commands
  (never `--release`); parity rows additionally cite the harness command,
  3Delight version, fixture names, and thresholds.
- Manual QA evidence (if any row falls back to it): exact steps,
  environment, and observed metrics.
