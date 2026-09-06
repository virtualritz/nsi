# Requirements Checklist: ɴsɪ Intermediate Representation

## Spec Quality

- [x] User stories are independently testable.
- [x] Acceptance criteria are observable.
- [x] Non-goals are explicit.
- [x] Risks are named.

## Contract Quality

- [x] Every important behavior has a contract row.
- [x] Every row is `Covered`, `Partial`, or `Open`. No row carries any
      other status; "Covered by design" was one, and the sentence it
      carried is now an invariant in `contracts/stream.md`.
- [x] `Covered` rows cite source evidence.
- [x] `Covered` rows cite test or manual QA evidence, by test name.
- [x] Required evidence is listed before work starts.

## Implementation Readiness

- [x] Tasks are small enough for single commits.
- [x] Each task names the contract row it closes, or the spec
      requirement it satisfies.

## Honesty Audit

Checks that this spec set does not overclaim.

- [x] No row is `Covered` on the strength of the suite passing alone;
      each names the test that proves it.
- [x] Known gaps are `Open`, not omitted. The ones that matter:
      - per-path transforms for an instanced node, and an `instances`
        node's own `transformationmatrices`;
      - `INTERPOLATE_LINEAR` on a sampled transform, which ɴsɪ provides
        and this crate ignores;
      - `recursive` delete;
      - the attribute vocabulary, legacy versus documentation draft.
- [x] `Covered` rows state what they do **not** prove where that is not
      obvious: `recorder_is_send_and_sync` proves the auto-trait and not
      the soundness argument behind it.
- [x] The fidelity gate's domain is stated as preconditions rather than
      implied to be general. Outside them the recorder and 3Delight
      differ by construction.
- [x] A rule that was chosen rather than quoted says so. Two rounds of
      review found this surface inventing semantics `nsi.pdf` already
      defines, so every requirement now quotes the specification where
      one exists, and `research.md` D8 records the entry that was itself
      wrong rather than quietly correcting it.
- [x] `stream.md` records that a missing 3Delight is a failed
      prerequisite, not a pass.
- [x] Accepted limitations are stated as such: grouping loss in
      `stream.md`, the `Callback` leak in R14, the `scene()` deadlock in
      R15, dropped call parameters in R16, visibility encoding left to
      backends in `resolution.md`.
