---
description: Produce the technical plan and supporting artifacts from the active spec.
---
Read the active spec and `.specify/memory/constitution.md`. Refuse to plan while
`[NEEDS CLARIFY]` markers remain -- run `/clarify` first.

From the spec, produce in the active feature directory:

- `plan.md` -- implementation approach, gates, and the artifact checklist.
- `research.md` -- decisions, rejected alternatives, references.
- `data-model.md` -- entities, ownership, wire formats, migrations.
- `contracts/*.md` -- one file per behavior surface, each with a contract matrix
  (`Covered` / `Partial` / `Open`) and a `Required Evidence Before Marking
  Complete` section.

Name the owning package or crate for any logic shared across runtimes or
languages. Check the plan against the constitution review gates. Do not edit
production code or write tasks; that is `/tasks`.
