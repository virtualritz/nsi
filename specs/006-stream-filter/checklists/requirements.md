# Requirements Checklist

- [x] Every user story in `spec.md` has a test or an example naming it.
- [x] The contract matrix has no `Covered` row without exact test
      evidence, and the two rows that are not `Covered` say what would
      make them so.
- [x] Each load-bearing test was falsified: the writer was mutated and
      the intended test reddened. Three mutations were run -- dropping
      a `Create`'s parameters, passing a redundant `action` through,
      and skipping a parameter name -- each reddening exactly one test.
- [x] The oracle is used where it can be: `renderdl -cat` reads the
      filter's output, rather than the crate only reading itself.
- [x] Non-goals are stated, with the reason rather than "later".
- [x] No public item added without documentation; rustdoc runs with
      `-D warnings`.
- [x] Both feature configurations of `nsi-intermediate` build, test
      and lint clean.
- [ ] A measurement of memory over a long stream. Structural argument
      only, recorded as `Partial`.
