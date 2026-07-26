---
description: Audit the active feature artifacts for cross-document consistency. Read-only.
---
Read the active `spec.md`, `plan.md`, `data-model.md`, `contracts/`, and
`tasks.md`.

Report, without editing anything:

- Spec requirements with no corresponding plan section or task.
- Plan or task content that implements behavior absent from the spec (scope
  creep).
- Contract rows with no task that produces their required evidence.
- Contract rows marked `Covered` whose evidence is not actually present.
- Shared logic named in the spec but not assigned an owning package or crate.
- Any remaining `[NEEDS CLARIFY]` markers.

Output a short findings list ordered by severity, then recommend the next
command (`/clarify`, `/plan`, or `/tasks`) to close the gaps. Do not edit any
file.
