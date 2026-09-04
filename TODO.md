# `nsi` -- To Do

Carried over from the (now removed) safety-audit notes; these are the items
that were never closed.

- **Understand the triple-`Box` callback pattern.** Output-driver callbacks use
  `Box<Box<Box<dyn Fn…>>>` (`crates/nsi-ffi-wrap/src/output/mod.rs`). Double
  boxing segfaults, triple boxing works, and nobody knows why -- the fat-pointer
  rationale in the file's header comment only justifies a *double* box. Until
  this is understood the `Box::leak` calls that avoid the resulting double-free
  have to stay.

- **Miri in CI.** No unsafe code path is currently validated under Miri.

- **Fuzz the FFI boundary.** Malformed input and error paths are untested.
