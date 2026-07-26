---
description: Generate a dependency-ordered task list from the plan and contracts.
---
Read the active `spec.md`, `plan.md`, `data-model.md`, and `contracts/`.

Write `tasks.md`:

- Group tasks by user story, then by contract row.
- Order by dependency. Mark tasks that can run in parallel -- no shared files,
  no ordering constraint -- with `[P]`.
- Give each task a stable id (`T001`, `T002`, ...) and name the exact files it
  touches.
- Pair each source task with the test or manual-QA task that produces its
  contract evidence. Tests come before the implementation they cover.
- End with a completion gate: run the project check/lint/test commands and
  confirm every ticked checkbox has evidence.

Do not edit production code.
