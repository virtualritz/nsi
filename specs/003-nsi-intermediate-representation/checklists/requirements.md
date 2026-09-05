# Requirements Checklist: ɴsɪ Intermediate Representation

## Spec Quality

- [x] User stories are independently testable.
- [x] Acceptance criteria are observable.
- [x] Non-goals are explicit.
- [x] Risks are named.

## Contract Quality

- [x] Every important behavior has a contract row.
- [x] Every row is `Covered`, `Partial`, or `Open`.
- [x] `Covered` rows cite source evidence.
- [x] `Covered` rows cite test or manual QA evidence.
- [x] Required evidence is listed before work starts.

## Implementation Readiness

- [x] Tasks are small enough for single commits.
- [x] Each task names the contract row it closes.

## Honesty Audit

Checks that this spec set does not overclaim.

- [x] No row is `Covered` on the strength of the suite passing alone;
      each names the test that proves it.
- [x] Known gaps are `Open`, not omitted. Three matter:
      motion-sampled transforms are unresolved, `disconnect` is untested,
      and `Reference` omission from the stream is an assumption.
- [x] `stream.md` records that a missing 3Delight is a failed
      prerequisite, not a pass.
- [x] Accepted limitations are stated as such: grouping loss in
      `stream.md`, visibility encoding left to backends in
      `resolution.md`.
