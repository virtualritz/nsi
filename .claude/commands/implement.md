---
description: Execute the active task list one user story or contract row at a time, evidence before ticking.
---
Read `.blueprints/domain/spec-driven-development.md`, the active `tasks.md`, and
the relevant `contracts/` file before touching code.

For each task, in dependency order:

1. Work one user story or one contract row at a time.
2. Read the contract row before editing the code it governs.
3. Derive tests from the contract preconditions, postconditions, and
   invariants. Write the test before the implementation.
4. Implement the change.
5. Run the evidence commands listed in the contract's `Required Evidence Before
   Marking Complete` section.
6. Only after the evidence passes: mark the contract row `Covered` and tick the
   task checkbox.

If implementation and spec disagree, stop -- update the spec via `/specify` or
reject the change. Never tick a checkbox or contract row before its evidence
runs green.
