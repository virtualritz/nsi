# Tasks: Parsing A Stream In Parallel

- [x] T1 Render the connection-order question before choosing keys.
      *Evidence:* research D3.
- [x] T2 Scan, phases, barriers: `parallel.rs`. *Evidence:*
      `tests/parallel.rs`, 16 tests.
- [x] T3 Same errors. *Evidence:* the six `*_is_the_same_error` tests.
- [x] T4 Falsify every ordering rule. *Evidence:* the contract's
      Falsified notes.
- [x] T5 Measure, null sink and 3Delight. *Evidence:* contract.
- [x] T6 Speed up the scan once it showed as the bottleneck. *Evidence:*
      research D5.
- [x] T7 CI. *Evidence:* `rust.yml`.
- [ ] T8 A parallel scan, if more speed is wanted. See the contract's
      Open section.
