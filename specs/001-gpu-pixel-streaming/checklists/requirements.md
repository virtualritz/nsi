# Requirements Checklist: GPU-Resident Pixel Streaming

Ticked items cite the document that satisfies them; everything else stays
unticked until its evidence exists.

## Spec Quality

- [x] User stories are independently testable (`spec.md` -- each story has
  its own contract rows and test commands).
- [x] Acceptance criteria are observable (`spec.md` -- given/when/then over
  acquirable images, errors, counters).
- [x] Non-goals are explicit (`spec.md` "Non-Goals").
- [x] Risks are named (`spec.md` "Risks").
- [x] All `[NEEDS CLARIFY]` markers resolved (`spec.md` -- four RESOLVED
  entries dated 2026-07-26; none remain).

## Contract Quality

- [x] Every important behavior has a contract row
  (`contracts/attribute-vocabulary.md`,
  `contracts/publication-lifecycle.md`).
- [x] Every row is `Covered`, `Partial`, or `Open` (2026-07-26: 16
  `Covered`, 1 `Partial`, 1 `Open` across both matrices).
- [x] `Covered` rows cite source evidence (`crates/nsi-stream/src/`
  symbols per row, 2026-07-26).
- [x] `Covered` rows cite test or manual QA evidence (exact
  `cargo test -p nsi-stream` commands per row, run 2026-07-26).
- [x] Required evidence is listed before work starts (both contract files).

## Implementation Readiness

- [x] Tasks are small enough for single commits (`tasks.md`; verified
  during the 2026-07-26 `/analyze` pass).
- [x] Persistence and migration behavior is documented (`data-model.md` --
  versioned wire contract, no disk state).
- [x] Shared logic ownership is named (`nsi-stream` crate,
  `research.md` D5).
