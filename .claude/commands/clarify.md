---
description: Resolve ambiguities in the active spec by asking structured questions, then record the answers.
---
Read the active spec named in `.specify/feature.json`.

1. Collect every `[NEEDS CLARIFY: ...]` marker plus any ambiguity you find in
   acceptance criteria, data ownership, persistence, or shared-logic
   boundaries.
2. Ask the user the open questions as a short numbered list. Ask only what
   changes the spec or plan -- skip cosmetic detail.
3. Record each answer under a `## Clarifications` section in `spec.md`, then
   remove the resolved `[NEEDS CLARIFY]` markers.
4. If an answer changes behavior, update the affected user stories and
   acceptance criteria in the same pass.

Block `/plan` until no `[NEEDS CLARIFY]` markers remain. Do not edit production
code.
