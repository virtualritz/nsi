# Requirements Checklist: ɴsɪ Parsing

## Spec Quality

- [x] User stories are independently testable.
- [x] Acceptance criteria are observable.
- [x] Non-goals are explicit: `binarynsi`, rendering, a scene type.
- [x] Risks are named, including the one that matters most -- a grammar
      with no specification behind it.

## Contract Quality

- [x] Every important behavior has a contract row.
- [x] Every row is `Covered`, `Partial`, or `Open`, and each `Covered`
      row names the test that proves it.
- [x] Required evidence is listed before work starts.

## Implementation Readiness

- [x] Tasks are small enough for single commits.
- [x] Each task names the story or contract it serves.
- [x] The order is stated and justified: correctness gates before any
      optimisation, so the speed requirement cannot be met by a parser
      that is subtly wrong.

## Honesty Audit

- [x] The spec was written before the code, and every row started
      `Open`. What is `Covered` now became so by a named test, not by
      the code existing.
- [x] A requirement that turned out to be false was **restated, not
      quietly met**: R6 claimed no allocation per argument. A counting
      allocator measured three per node, and then five once the corpus
      included the string values the first one omitted. The parser was
      fixed to borrow, and the remaining cost -- three allocations per
      string argument, inside `nsi_ffi_wrap`'s owning `StringSlice` --
      is attributed rather than hidden.
- [x] The escape set is measured, not assumed. An earlier version
      decoded `\xHH`, which the renderer never writes, and rejected the
      three-digit octal it does -- so a stream with a tab in a string
      could not be read.
- [x] The grammar is recorded as *observed*, with the probe that
      established each rule, because the ɴsɪ specification does not give
      one. The decisive observation -- a whole scene on one line parses
      -- is written down, since it is what makes a line-based parser
      wrong.
- [x] An unestablished rule is `Open`, not assumed: `NSI_PATH_`
      replacement is documented as unknown in both trigger and syntax.
- [x] The Lua feature's cost is stated plainly: reading a script runs
      it.
- [x] The performance requirement carries a measurement method, a
      recorded figure and a machine, rather than the word "fast".
