# Plan: Fuzzing The Untrusted Boundaries

## Approach

One `cargo-fuzz` target, `parse_stream`, in `crates/nsi-parse/fuzz`.
It hands arbitrary bytes to `nsi_parse::parse_stream` with a sink that
accepts every call and keeps nothing, and asserts only that the call
returns. The fuzz crate is its own workspace, so `libfuzzer-sys` and the
sanitizer profile stay out of the parent's build.

Seeds are ten small hand-written streams under `fuzz/seeds/`, one per
statement kind the grammar has, so the fuzzer starts inside the grammar.
The corpus it grows lives under `fuzz/corpus/`, which is ignored:
it is the fuzzer's working state, not a fixture.

## Gates

1. **The target builds**, in CI, on every push -- `fuzz-build` in
   `.github/workflows/rust.yml`.
2. **A run finds nothing**, by hand, per `quickstart.md`. Not in CI:
   fuzzing has no natural end, and a time-boxed run there is a flaky
   test.
3. **Any crash becomes a test** in `nsi-parse` with the reproducing
   bytes, before the fix -- so the test reddens first.

## Artifacts

- [x] `spec.md`
- [x] `plan.md`
- [x] `research.md`
- [x] `data-model.md`
- [x] `contracts/fuzzing.md`
- [x] `quickstart.md`
- [x] `tasks.md`
- [x] `checklists/requirements.md`
