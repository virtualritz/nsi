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
- [x] T9 **A backend drives it.** `nsi-moonray` builds against the
      current API, and its interactive path is this crate's:
      `session.rs` calls `Scene::affected(&changes)` and hands the
      result to its own `apply_affected`, which narrows an edit to the
      objects it touched and falls back to a full re-apply for anything
      it cannot narrow. `incremental::a_session_runs_a_synchronise_loop`
      and `a_synchronise_is_measured_not_assumed` are where it is
      exercised -- the second measures the cost rather than assuming
      it, which is what turned up the BVH-only tier being unreachable
      and produced the report in `upstream/`.

      So the *unblocking* is what this task was waiting for, and it has
      happened. What a second backend would add is independent
      confirmation, and `nsi-mitsuba` is where that goes.
- [ ] T10 Decide whether overlapping roots are worth collapsing, with a
      measurement rather than a guess. `contracts/changes.md` carries
      it as `Partial`.
