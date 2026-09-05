# Tasks: ɴsɪ Intermediate Representation

Landed tasks are kept as the record of what shipped. Open tasks are the
`Partial` and `Open` contract rows, and nothing else.

## User Story 1: Record An ɴsɪ Scene (P1)

- [x] T1.1 `impl ParamValue for Arg` upstream in `nsi-ffi-wrap`.
      Evidence: `param_value::tests`, 4 cases.
- [x] T1.2 `impl Nsi for Context` upstream.
      Evidence: `nsi_impl::tests`, 2 cases.
- [x] T1.3 `OwnedArg` / `OwnedData` / `HostPtr`.
      Evidence: `owned::tests`, 5 cases.
- [x] T1.4 `Scene` node and attribute tables.
      Evidence: `scene::tests`, 7 cases.
- [x] T1.5 `Recorder` implementing `Nsi`.
      Evidence: `recorder::tests`, 5 cases.
- [ ] T1.6 Test `Nsi::delete` through the `Recorder`, not `Scene`.
      Gate: `contracts/recording.md` `delete` row to `Covered`.
- [ ] T1.7 Test `delete_attribute` removing from a time sample.
      Gate: `contracts/recording.md` `delete_attribute` row.
- [ ] T1.8 Test `disconnect`, including an unmapped `to_attr`.
      Gate: `contracts/recording.md` `disconnect` row, currently `Open`.
- [ ] T1.9 Decide `evaluate`: test the no-op, or record the decision to
      leave procedurals until a backend needs them.

## User Story 2: Know What A Connection Means (P1)

- [x] T2.1 Exhaustive `classify` over `to_attr`.
      Evidence: `tests/classifier.rs`, 7 cases.
- [ ] T2.2 Extend the roundtrip fixture to every non-shader edge class.
      Gate: `contracts/classification.md` inverse row.

## User Story 3: Resolved Facts (P1)

- [x] T3.1 `world_transform` with row-vector composition and a cycle
      budget. Evidence: `resolve::tests`, 6 cases.
- [x] T3.2 `geometry_binding`. Evidence: `binding_tests`, 4 cases.
- [x] T3.3 `render_outputs`. Evidence: `output_tests`, 5 cases.
- [x] T3.4 `instance_sources`. Evidence: `instance_tests`, 2 cases.
- [ ] T3.5 **Motion-sampled transforms.** `world_transform` reads static
      attributes only, so a motion-blurred scene resolves to its static
      transform. Decide the API, then test a two-sample chain.
      Gate: `contracts/resolution.md`, largest known gap.
- [ ] T3.6 Two-screen fixture.
- [ ] T3.7 Test that a `MatrixF32` transform is skipped deliberately.

## User Story 4: Fidelity (P2)

- [x] T4.1 `write_stream`, format read from live 3Delight output.
      Evidence: `stream_roundtrip`, 1 case, against 3Delight 2.9.207.
- [ ] T4.2 Add matrices to the roundtrip fixture.
- [ ] T4.3 Determine whether 3Delight emits `Reference` arguments.
      Gate: `contracts/stream.md` `Reference` row, currently an
      assumption.
