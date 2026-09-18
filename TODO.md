# `nsi` -- To Do

- **Fuzz `nsi-ffi-wrap`'s C boundary.** The parser is fuzzed
  (`specs/008-fuzzing`: two million executions, no finding); the C
  boundary is its T5 and needs a sanitizer run.

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

- **Comment blocks not ending in punctuation: done.** 178 blocks
  fixed across 23 files; the scan now finds none. Two classes of false
  positive had to be excluded first, and the second was caught only
  because the diff was read: section banners (`// ─── Publication ───`)
  are not sentences, and **intra-doc link definitions**
  (``[`Nsi`]: nsi_trait::Nsi``) break outright when a period lands
  inside the target. A blanket pass over "comments not ending in a
  period" silently breaks documentation links.

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
