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
- [x] Every row is `Covered`, `Partial`, or `Open` (all currently `Open`).
- [ ] `Covered` rows cite source evidence (none yet -- no implementation).
- [ ] `Covered` rows cite test or manual QA evidence (none yet).
- [x] Required evidence is listed before work starts (both contract files).

## Implementation Readiness

- [ ] Tasks are small enough for single commits (`tasks.md` drafted; verify
  during `/analyze` after clarifications).
- [x] Persistence and migration behavior is documented (`data-model.md` --
  versioned wire contract, no disk state).
- [x] Shared logic ownership is named (`nsi-stream` crate,
  `research.md` D5).
