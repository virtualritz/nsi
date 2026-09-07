# Feature Spec: What Changed Since The Last Synchronise

## Context

ɴsɪ has a word for an interactive edit: `NSIRenderControl` with
`"action" "synchronize"`, which the specification defines as "apply all
the **buffered** calls to scene's state". A host drags a slider, one
attribute on one node changes, and the render restarts from what the
renderer already has.

This crate recorded a scene and answered questions about the whole of
it. That is right for a batch flush and useless for a viewport: a
backend synchronising a live renderer had to diff whole scenes, and
`Scene::clone` deep-copies every vertex buffer, so a slider drag paid a
full copy of the scene per tick.

Observed on 3Delight, interactively: edits made without synchronising
change nothing and report nothing; the synchronise then reports
`Restarted` followed by `Synchronized` **once**, for the whole batch.
So the interval between two synchronises is exactly one batch of edits,
and that interval is what this feature records.

The requirement came from `nsi-moonray`'s `002-interactive-updates`,
whose spec says the backend "holds no ɴsɪ graph knowledge" and that
working out which objects an ɴsɪ edit touches belongs here, beside
composition and dissolution. Mitsuba cannot edit a live scene, which is
why nothing had forced it -- the same position motion-sample resolution
was in before a backend needed it.

## User Stories

### User Story 1: What Changed (P1)

As a backend driving a live renderer, I want to ask what changed since
I last synchronised, so that I can apply an edit rather than rebuild a
scene.

**Acceptance Criteria**

- Given a recorded scene, when attributes are set and connections made,
  then `take_changes` names them and clears the record.
- Given forty sets of one attribute, then the record holds one entry:
  it is a net record, not a log of calls.
- Given a handle created and deleted in the same interval, then only
  the deletion is reported -- a consumer that never saw the node has
  nothing to undo.

### User Story 2: What To Look At Again (P1)

As the same backend, I want to know which nodes' *resolved* answers may
have moved, because ɴsɪ handles are not the renderer's objects: a
`transform` has no counterpart there, and one geometry under two
parents is several objects.

**Acceptance Criteria**

- Given a transform edit, then everything below it is reported as
  affected, and nothing on another branch is.
- Given a shader parameter edit, then the shader is reported and no
  geometry is: that edit costs no geometry work.
- Given an `attributes` node bound to a `set`, then the set's members
  are reported.

### User Story 3: An Answer That Scales (P1)

As a backend for a production scene -- millions of instances, not a
sphere on a checkerboard -- I want the answer to cost what the edit
costs, not what the scene costs.

**Acceptance Criteria**

- Given one transform edit in a 200 000-node scene, then computing the
  affected set is measured in microseconds, not milliseconds.

## Non-Goals

- **A minimal affected set.** ɴsɪ's precedence can make an edit
  invisible; deciding that here would mean remembering every answer this
  crate ever gave, which duplicates the renderer's own change mask.
  The answer is candidates.
- **Path-precise dirtying.** One geometry under two parents reports as
  one handle, not per placement. Correct and coarse; precision is an
  optimisation to ask for with a measurement.
- **Values in the record.** The scene holds the current value.
- **Executing procedurals.** ɴsɪ says a procedural is re-executed on
  edit and its nodes deleted; this crate records `Evaluate` and does not
  execute it, so that belongs to whoever does.
- **Clearing on the caller's behalf.** `take_changes` is called from the
  backend's own `NSIRenderControl`, under its own lock.

## Requirements

- R1: The record is **net** -- one entry per fact, not per call.
- R2: The record carries no values.
- R3: What a mutator destroys is recorded before it destroys it: a
  delete's edges, a `.all` disconnect's matches, and a repeated
  `connect` that replaces an edge's arguments in place.
- R4: The affected set is over-approximate and never under-approximate.
  `changed ⊆ affected` is the invariant.
- R5: The affected set is keyed by handle and reported as **roots**:
  a node and everything below it.
- R6: Pending changes are not part of a scene's identity.

## Risks

- **Under-approximation is silent.** A node whose answer moved and which
  the affected set does not name makes a backend synchronise everything
  except the thing that changed, and render the old state with no error.
  This is the failure mode the property gate exists for.
- **A gate that cannot fail.** That gate was widened three times before
  it could catch anything; see `contracts/changes.md`.
