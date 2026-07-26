---
description: Create or refresh spec.md for a feature surface from a plain-language description.
argument-hint: <feature description>
---
Read `.blueprints/domain/spec-driven-development.md` and
`.specify/memory/constitution.md` first.

Feature description: $ARGUMENTS

1. Determine the active feature directory from `.specify/feature.json`. If the
   description names a new surface, create `specs/NNN-feature-name/` (next free
   `NNN`), copy `.blueprints/templates/spec-driven/feature/` into it, and point
   `.specify/feature.json` at the new directory.
2. Fill `spec.md` from the template: user stories with acceptance criteria,
   functional and non-functional requirements, non-goals, and risks. Slice by
   independently testable user stories.
3. Write only behavior -- the "what" and "why". Do not choose a tech stack or
   design internals here; that is `/plan`.
4. Mark every unresolved ambiguity inline as `[NEEDS CLARIFY: ...]`. Do not
   guess. Hand off to `/clarify` if any remain.

Do not edit production code. The output is the spec only.
