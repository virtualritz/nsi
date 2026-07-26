# Implementation Plan: Shading Profile

**Branch**: `claude/nsi-api-realtime-2okw3y` | **Date**: `2026-07-26` |
**Spec**: `specs/002-shading-profile/spec.md`.

## Summary

Define the versioned closure/node vocabulary for realtime ɴsɪ shading and
implement it as dual-target node definitions: ᴏsʟ references for offline
renderers and SPIR-V emitters for realtime backends, with a loud validator
and an image-comparison parity harness against 3Delight. Sketch status:
this surface is specced second but implemented after feature 001; three
the former `[NEEDS CLARIFY]` markers were resolved 2026-07-26 (spec R2/R3/R4).

## Technical Context

**Language/Version**: Rust, workspace toolchain. ᴏsʟ sources for the
reference implementations.

**Primary Dependencies**: `nsi-trait`; SPIR-V emission (`rspirv` or
MaterialX shadergen + glslang, pending R3/R4); optional `materialx` feature
for interchange (US5); 3Delight + `DELIGHT` for parity renders.

**Storage**: none on disk beyond fixtures. Wire surfaces: the
`nsi-profile:<node>@<version>` scheme and the ParameterBlock layout
(`data-model.md`), semantic-versioned.

**Testing**: `cargo test -p nsi-profile` (never `--release`); parity via
the repo's expected-image machinery (`RUST_TEST_UPDATE=1` gated on human
approval).

**Target Platform**: platform-neutral (SPIR-V artifacts); parity harness
runs where 3Delight runs (Linux first, matching feature 001).

**Performance Goals**: parameter edits must not trigger re-translation
(R6/edit classification); translation itself is off the frame loop.

**Constraints**: no ɴsɪ API/node additions (R7); pure-Rust workspace
preference weighs on the R3 MaterialX-dependency decision; profile v1
excludes runtime-callback constructs (Non-Goals).

## Constitution Check

- Source-of-truth: `.specify/feature.json` remains
  `specs/001-gpu-pixel-streaming` (001 implements first); this surface
  activates when the pointer moves here.
- Required artifacts: all eight present in this directory.
- Evidence policy: `contracts/profile-conformance.md` includes
  `Required Evidence Before Marking Complete`; 2026-07-26: 6 rows
  `Covered` with evidence, 3 `Open` (parity, emission parity, MaterialX).
- Scope: the three formerly-open markers (closure list, MaterialX
  nodedefs vs native, WGSL target) were resolved 2026-07-26 and recorded
  in `spec.md` R2/R3/R4.

## Project Structure

```text
specs/002-shading-profile/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── tasks.md
├── checklists/
│   └── requirements.md
└── contracts/
    └── profile-conformance.md

crates/nsi-profile/           (new, this feature; name tentative)
├── src/
│   ├── lib.rs                registry, versioning, resolution
│   ├── validate.rs           US3 validator
│   ├── translate.rs          network → NetworkModule
│   ├── emit/                 SPIR-V emitters
│   └── nodes/                NodeDefs + ᴏsʟ references
├── osl/                      ᴏsʟ reference sources
└── tests/                    contract-derived tests + parity fixtures
```

## Execution Rules

1. Resolve the three `[NEEDS CLARIFY]` markers before implementation
   (done 2026-07-26, recorded in `spec.md` R2/R3/R4).
2. Work one user story or one contract row at a time.
3. Add or update tests from the contract invariants before ticking rows.
4. Mark rows `Covered` only after the listed evidence commands ran.

## Artifact Checklist

- [ ] Active feature pointer is updated (deliberately not -- 001 first).
- [x] Required artifact set exists.
- [x] Each contract file has `Required Evidence Before Marking Complete`.
