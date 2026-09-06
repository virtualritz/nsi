# Contract: Throughput

## Scope

Covers R6, R7 and R8: what the parser is allowed to allocate, and how
fast it is.

## Why This Contract Exists

"Fast" is not a property a test can assert without a number. This
contract exists so the number is written down, measured the same way
each time, and compared rather than felt.

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| The parser does not allocate as a scene grows | Covered | `value.rs` `Scratch` cleared and never freed; names and string values `Cow`-borrowed from the input; the argument list a `SmallVec` on the stack | `allocation::allocation_behaviour`: **5 allocations for 100 nodes and 5 for 1000**, asserted equal. It was 3.0 per node before the borrowing and the stack list | -- |
| A string argument's cost is attributed, not hidden | Covered | `nsi_ffi_wrap::StringSlice` owns a `Vec<CString>` plus a pointer vector | Same test: **3002 allocations for 1000 string arguments**, three each, inside the argument type rather than the parser. An earlier corpus omitted string values entirely, so the figure it produced said nothing | -- |
| An unescaped string is borrowed | Covered | `lex.rs` `Quoted::Borrowed`, copying only when an escape is present | The allocation figure above is the proof: a corpus full of quoted handles and names allocates nothing per statement, which is only possible if they are borrowed | -- |
| Throughput is measured and recorded | Covered | -- | `throughput::throughput`: **175--183 MiB/s**, parsing 5.3 MiB of a 20 000-mesh corpus into a sink that discards. Release profile, x86-64 Linux. Debug is roughly an order of magnitude slower and is not comparable | -- |

## Recorded Figures

| What | Figure |
| --- | --- |
| Parser allocations, 100 nodes | 5 |
| Parser allocations, 1 000 nodes | 5 |
| String arguments, 1 000 | 3 002 (three each, in `nsi_ffi_wrap`) |
| Throughput | 175--183 MiB/s, release, x86-64 Linux |

A figure without its profile and machine is not a comparison, which is
why both are written down. Re-measure with
`cargo test -p nsi-parse --release --test throughput -- --ignored --nocapture`.

The allocation figures come from one test function on purpose. The
counter is a global, and `cargo test` runs tests in parallel threads, so
two of them count each other: split in two, the same measurement read
539 against 4910 and looked like a leak.

## Invariants

- Correctness gates land before any optimisation. A fast parser that is
  subtly wrong is worse than a slow one, and the round-trip gates are
  what stop the trade being made accidentally. That order held here: the
  gates were green before anything was measured, and the two
  optimisations that followed -- borrowing names, stacking the argument
  list -- changed no behaviour and were re-checked against the gates.
- The measurement is recorded with the machine and build profile, since
  a number without either is not a comparison.

## Required Evidence Before Marking Complete

- The corpus generator, the counting allocator test, and a recorded
  figure.
