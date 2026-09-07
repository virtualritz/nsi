# Tasks: What Changed Since The Last Synchronise

- [x] T1 `Changes` and `Scene::take_changes`: net, valueless, clearing.
      Evidence: `scene::tests::the_record_is_net_not_a_log`.
- [x] T2 Record what the mutators destroy -- a delete's edges, a `.all`
      disconnect's matches, a repeated `connect`'s replaced arguments.
      Evidence: three named tests in `scene::tests`, each falsified.
- [x] T3 `Scene::affected`: the inverse walks, over `EdgeKind`
      exhaustively. Evidence: six named tests plus the property gate.
- [x] T4 The property gate `changed ⊆ affected`. Widened three times
      before it could fail; see `contracts/changes.md`.
- [x] T5 Adversarial review round, and its findings applied: the
      instancer arm, the output-chain buckets, the net edge lists, the
      `created` netting, the exhaustive match.
- [x] T6 Roots rather than an enumeration, measured.
- [x] T7 `Affected` borrows rather than owns.
- [x] T8 Reconcile `003`'s `data-model.md` with the `Scene` this adds
      to.
- [ ] T9 A backend drives it. Blocked on `nsi-moonray` building against
      the current API at all; see `plan.md`.
- [ ] T10 Decide whether overlapping roots are worth collapsing, with a
      measurement rather than a guess. `contracts/changes.md` carries
      it as `Partial`.
