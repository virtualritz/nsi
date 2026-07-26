# Tasks: Shading Profile

Dependency-ordered; implementation starts after feature 001 (the active
pointer stays on 001 until then). Tasks S1--S2 are blocked on `/clarify`
resolving the three `[NEEDS CLARIFY]` markers in `spec.md`.

## Setup

- [ ] S1: Resolve `[NEEDS CLARIFY]` markers (v1 closure list, MaterialX
  nodedefs vs ɴsɪ-native, WGSL target) and record answers in `spec.md`.
  Evidence: spec with markers removed.
- [ ] S2: Freeze profile v1: ClosureDef and NodeDef tables in
  `data-model.md`. Evidence: `/analyze` shows no drift.
- [ ] S3: Scaffold `crates/nsi-profile` (registry, versioning, typed
  errors). Evidence: `cargo build -p nsi-profile`.

## User Story 1 -- Versioned Closure Vocabulary (P1)

- [ ] S4: `nsi-profile:<node>@<version>` resolution + version negotiation.
  Evidence: `resolve_scheme`.
- [ ] S5: Registry completeness check (every NodeDef has both targets).
  Evidence: `nodedef_completeness`.

## User Story 3 -- Loud Validation (P2, pulled early: gates everything)

- [ ] S6: Validator + CI wiring, fixture scenes (conforming + violating).
  Evidence: `validator_violations`, `validator_clean`.

## User Story 2 -- Offline/Realtime Parity (P1)

- [ ] S7: ᴏsʟ reference set for v1 nodes. Evidence: fixture scenes render
  via 3Delight (`DELIGHT` set).
- [ ] S8: SPIR-V emitters for v1 nodes; ParameterBlock layout golden
  files. Evidence: `parameter_block_layout`.
- [ ] S9: Parity harness (image comparison, per-fixture thresholds) in CI.
  Evidence: parity rows of the contract.
- [ ] S10: Edit classification (parameter update vs re-translate).
  Evidence: `edit_classification`.

## User Story 4 -- Emission Parity (P2)

- [ ] S11: Light-rig fixtures (area, spot, point, directional, HDR
  environment) through the parity harness. Evidence: emission parity row.

## User Story 5 -- MaterialX Interchange (P3)

- [ ] S12: `materialx` feature: import mapping for the documented subset.
  Evidence: `mtlx_roundtrip`.
