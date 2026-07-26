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
- [ ] All `[NEEDS CLARIFY]` markers resolved (3 remain: v1 closure list,
  MaterialX nodedefs vs native, WGSL target -- blocked on `/clarify`).

## Contract Quality

- [x] Every important behavior has a contract row
  (`contracts/profile-conformance.md`).
- [x] Every row is `Covered`, `Partial`, or `Open` (all currently `Open`).
- [ ] `Covered` rows cite source evidence (none yet -- no implementation).
- [ ] `Covered` rows cite test or manual QA evidence (none yet).
- [x] Required evidence is listed before work starts (contract file).

## Implementation Readiness

- [ ] Tasks are small enough for single commits (`tasks.md` drafted;
  verify during `/analyze` after clarifications).
- [x] Persistence and migration behavior is documented (`data-model.md` --
  versioned scheme + ParameterBlock wire format, no disk state).
- [x] Shared logic ownership is named (`nsi-profile` crate owns both
  targets; `research.md` D1, constitution VII).
