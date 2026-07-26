# Tasks: Shading Profile

Dependency-ordered; implementation starts after feature 001 (the active
pointer stays on 001 until then). Tasks S1--S2 are blocked on `/clarify`
resolving the three `[NEEDS CLARIFY]` markers in `spec.md`.

## Setup

- [x] S1: Resolve `[NEEDS CLARIFY]` markers (v1 closure list, MaterialX
  nodedefs vs ɴsɪ-native, WGSL target) and record answers in `spec.md`.
  Evidence: spec with markers removed -- three RESOLVED entries dated
  2026-07-26 in Requirements R2/R3/R4.
- [x] S2: Freeze profile v1: ClosureDef and NodeDef tables in
  `data-model.md`. Evidence: "Profile v1 Tables (Frozen 2026-07-26)"
  section in `data-model.md`, mirrored from the machine-readable registry
  (`crates/nsi-profile/src/v1.rs`), cross-checked against spec R2 list and
  contract rows while recording evidence 2026-07-26.
- [x] S3: Scaffold `crates/nsi-profile` (registry, versioning, typed
  errors). Evidence: `cargo build -p nsi-profile` -- ok (2026-07-26).

## User Story 1 -- Versioned Closure Vocabulary (P1)

- [x] S4: `nsi-profile:<node>@<version>` resolution + version negotiation.
  Evidence: `cargo test -p nsi-profile resolve_scheme` -- ok (2026-07-26).
- [x] S5: Registry completeness check (every NodeDef has both targets).
  Evidence: `cargo test -p nsi-profile nodedef_completeness` -- ok
  (2026-07-26). Caveat recorded in the contract row: the ᴏsʟ references
  are not yet compiled by `oslc` (unavailable on this box); required
  before S7/US2 parity claims.

## User Story 3 -- Loud Validation (P2, pulled early: gates everything)

- [x] S6: Validator + CI wiring, fixture scenes (conforming + violating).
  Evidence: `cargo test -p nsi-profile validator_violations` and
  `validator_clean` -- ok (2026-07-26); CI wiring: workspace-crate test
  job added to `.github/workflows/rust.yml` (same session).

## User Story 2 -- Offline/Realtime Parity (P1)

- [ ] S7: ᴏsʟ reference set for v1 nodes. Evidence: fixture scenes render
  via 3Delight (`DELIGHT` set).
- [x] S8: GPU emitters for v1 nodes (per the R4 resolution: GLSL 4.60
  source of record behind the `GpuEmitter` trait, SPIR-V compile is a
  backend step); ParameterBlock layout golden files. Evidence:
  `cargo test -p nsi-profile parameter_block_layout` -- ok (2026-07-26),
  golden file `crates/nsi-profile/tests/golden/parameter_block_v1.txt`.
- [ ] S9: Parity harness (image comparison, per-fixture thresholds) in CI.
  Evidence: parity rows of the contract.
- [x] S10: Edit classification (parameter update vs re-translate).
  Evidence: `cargo test -p nsi-profile edit_classification` -- ok
  (2026-07-26).

## User Story 4 -- Emission Parity (P2)

- [ ] S11: Light-rig fixtures (area, spot, point, directional, HDR
  environment) through the parity harness. Evidence: emission parity row.

## User Story 5 -- MaterialX Interchange (P3)

- [ ] S12: `materialx` feature: import mapping for the documented subset.
  Evidence: `mtlx_roundtrip`.
