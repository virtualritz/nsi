# `nsi` -- To Do

- **Fuzz the FFI boundary.** Malformed input and error paths are untested.

## Blueprint Compliance Backlog

Measured across the workspace on 2026-09-07, after `nsi-intermediate`,
`nsi-parse`, `nsi-trait` and `nsi-ffi-wrap` were brought to the `ahash`,
`parking_lot` and `SAFETY`-comment rules in `AGENTS.md`. The counts are
what a scan found, not an estimate; re-run the scans before working on a
row, since they move.

- **`SAFETY` comments on library `unwrap`/`expect`: done, bar a
  decision.** The scan that found about sixty sites now finds six, and
  each of those is a *reachable* panic with a `# Panics` section rather
  than an invariant-guaranteed unwrap, so a `SAFETY` comment would be
  the wrong annotation: `Token::new`, `Handle::new`, `String::new` and
  `StringSlice::new` panic on an interior NUL, which C cannot carry,
  and `NSI_API` panics when the renderer's library will not load.
  Turning those into `Result` is the blueprint's preference and an API
  change on a published crate, so it needs approval first.

  Most of the backlog was removed rather than commented:
  `nsi-stream` moved to `parking_lot`, and twenty-two of the sites
  existed only to handle lock poisoning, which `parking_lot` does not
  have.

- **Comment blocks not ending in punctuation: 302**, concentrated in
  `nsi-ffi-wrap` (`tests/safety.rs` 40, `tests/materials.rs` 24,
  `src/output/mod.rs` 21, `src/c_api.rs` 18) and `nsi-stream`
  (`ring.rs` 11, `bridge/mod.rs` 10). Check the *last* line of each
  block, not every line -- a sentence continued across lines is fine.

- **Files over the 1,000-line target**, none over the 2,000 soft limit:
  `nsi-ffi-wrap/src/output/mod.rs` 1434, `nsi-ffi-wrap/src/argument.rs`
  1416, `nsi-intermediate/src/scene/mod.rs` 1371. Signals to plan a
  split along responsibility, not an urgent violation.
  `nsi-intermediate/src/resolve/tests.rs` is 5287 lines; tests are
  exempt from the budget, but it is worth splitting per concern.

- **Clean, as of the same scan:** no `std::collections::HashMap`/
  `HashSet` anywhere, no `std::sync` lock primitives in library code,
  and no genuine inline-path violations (the ones a naive grep reports
  are module-level `use` inside `mod` blocks, which is correct).

## Archived Histories

`refs/archive/nsi-record/master` and `.../crate-only` hold the ten
commits of `nsi-record`, the standalone crate `nsi-intermediate` grew
out of -- the split, the connection classifier, the node tables, the
`Recorder`, the stream emitter and the resolver. It lived in
`~/code/crates/nsi-record` with no remote, so the history existed in
one directory; the directory is gone and the refs are the record. They
are local refs: `git push origin refs/archive/nsi-record/master` if
this repo should carry them upstream too. A `git bundle` copy is at
`~/.local/share/nsi-archive/nsi-record-2026-09-07.bundle`.
