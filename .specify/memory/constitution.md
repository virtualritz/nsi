# ɴsɪ Project Spec Constitution

Adapted from `.blueprints/templates/spec-driven/constitution.md` for the ɴsɪ
Rust workspace.

## Core Principles

### I. Source Of Truth First

Feature behavior is governed by the active spec directory named in
`.specify/feature.json`. Code, plans, TODOs, and agent claims must point back
to contract files. If implementation and spec disagree, update the spec or
reject the implementation before continuing.

### II. Contract Evidence Is Required

Every feature surface needs contracts with a matrix row for each important
behavior. Each row is `Covered`, `Partial`, or `Open`. `Covered` requires
source evidence plus executable test evidence or explicit manual QA evidence.
Untested documentation is not evidence.

### III. Small User Stories Beat Broad Architecture Docs

Specs are sliced by independently testable user stories. A single session
should target one user story or one contract gap. Architecture summaries may
exist as context, but they are not a substitute for acceptance criteria,
contracts, and tasks.

### IV. Tests Follow Contracts

Tests must be derived from contract preconditions, postconditions, invariants,
and acceptance criteria. Deterministic logic needs automated tests
(`cargo test`, never `--release` unless explicitly requested). Behavior that
requires a running renderer (3Delight, `DELIGHT` env var) needs automated
tests when practical and explicit manual QA steps when not.

### V. Wire Formats And FFI Boundaries Are Product Behavior

Attribute vocabularies, exported OS handles, shared-memory layouts, and any
data crossing the FFI or a process boundary are contract surfaces. Any change
must document compatibility, versioning, and failure mode. Silent fallback on
required identifiers is forbidden -- fail loudly with typed errors.

### VI. API Conformance Is A Contract

Anything claiming ɴsɪ conformance must be expressible through the standard
interface calls and node/attribute model. Renderer-specific behavior must be
carried by attributes that a conforming implementation may ignore, and each
such attribute must be named in a contract.

### VII. Shared Logic Has One Owner

Code needed by multiple crates in the workspace must have one named owner
crate. Temporary duplication must be documented as `Partial` with a removal
task.

## Required Feature Artifacts

Each feature surface must contain:

- `spec.md`.
- `plan.md`.
- `research.md`.
- `data-model.md`.
- `contracts/*.md`.
- `quickstart.md`.
- `tasks.md`.
- `checklists/requirements.md`.

## Review Gates

- `cargo build`, `cargo clippy`, and `cargo fmt --check` must pass before
  claiming readiness unless a known unrelated failure is documented with exact
  output.
- Relevant crate tests must run for every changed surface.
- Expected-image update commands (`RUST_TEST_UPDATE=1`) require explicit human
  approval.
- Plans and TODO checkboxes may be ticked only after their contract evidence
  is present.

## Governance

This constitution applies unless a branch-specific constitution explicitly
overrides it. Amendments require a commit changing this file, an explanation
in the PR or handoff, and updates to affected specs/templates.
