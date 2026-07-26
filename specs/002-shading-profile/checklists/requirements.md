# Requirements Checklist: Shading Profile

Ticked items cite the document that satisfies them; everything else stays
unticked until its evidence exists.

## Spec Quality

- [x] User stories are independently testable (`spec.md` -- each has its
  own contract rows and commands).
- [x] Acceptance criteria are observable (`spec.md` -- validation reports,
  image-comparison metrics, exit codes).
- [x] Non-goals are explicit (`spec.md` "Non-Goals", normative
  excluded-construct list).
- [x] Risks are named (`spec.md` "Risks").
- [x] All `[NEEDS CLARIFY]` markers resolved (`spec.md` -- three RESOLVED
  entries dated 2026-07-26 in R2/R3/R4; none remain).

## Contract Quality

- [x] Every important behavior has a contract row
  (`contracts/profile-conformance.md`).
- [x] Every row is `Covered`, `Partial`, or `Open` (2026-07-26: 6
  `Covered`, 3 `Open` -- parity and MaterialX rows await 3Delight/oslc).
- [x] `Covered` rows cite source evidence (`crates/nsi-profile/src/`
  symbols per row, 2026-07-26).
- [x] `Covered` rows cite test or manual QA evidence (exact
  `cargo test -p nsi-profile` commands per row, run 2026-07-26).
- [x] Required evidence is listed before work starts (contract file).

## Implementation Readiness

- [x] Tasks are small enough for single commits (`tasks.md`; verified
  during the 2026-07-26 `/analyze` pass).
- [x] Persistence and migration behavior is documented (`data-model.md` --
  versioned scheme + ParameterBlock wire format, no disk state).
- [x] Shared logic ownership is named (`nsi-profile` crate owns both
  targets; `research.md` D1, constitution VII).
