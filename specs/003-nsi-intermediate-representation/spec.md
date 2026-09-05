# Feature Spec: ɴsɪ Intermediate Representation

## User Stories

### User Story 1: Record An ɴsɪ Scene Without A Renderer (P1)

As a renderer-backend author, I want to receive ɴsɪ calls and hold the
resulting scene in memory, so that I can build a backend without a
renderer present and test it without one.

**Acceptance Criteria**

- Given a type implementing `nsi_trait::Nsi`, when a consumer issues
  `create`, `set_attribute` and `connect`, then the nodes, attributes
  and connections are retrievable afterwards.
- Given an argument of any ɴsɪ type, when it is recorded, then its
  payload is copied and remains valid after the call returns.
- Given a `Reference` argument, when it is recorded, then the host
  address is stored rather than the data it points at.
- Given a `Callback` argument, when it is recorded, then its address is
  stored and its payload leaks, because a recorder cannot reclaim one.
  See R14.

### User Story 2: Know What A Connection Means (P1)

As a backend author, I want each ɴsɪ connection classified by its
meaning, so that I do not map an output-routing edge onto a material
reference.

**Acceptance Criteria**

- Given a connection, when it is recorded, then it is classified by its
  destination attribute into exactly one known class.
- Given a destination attribute with no mapping, when it is recorded,
  then the call fails rather than defaulting to a guess.
- Given a source attribute of `Some("")`, when it is recorded, then it
  classifies as if it were `None`, because ɴsɪ documents the two as
  equivalent.

### User Story 3: Receive Resolved Facts, Not Graph Semantics (P1)

As a backend author, I want ɴsɪ's graph semantics applied for me, so
that every backend does not re-derive transform composition and
material binding.

**Acceptance Criteria**

- Given a chain of transform nodes, when a leaf's world transform is
  requested, then the chain is composed in ɴsɪ's row-vector order.
- Given `shader -> attributes -> geometry`, when a geometry's binding is
  requested, then the shader is resolved through the intermediate node.
- Given `attributes` bound to an ancestor transform, when a geometry's
  binding is requested, then the inherited binding is found.
- Given `outputdriver -> outputlayer -> screen -> camera`, when render
  outputs are requested, then one entry per screen is produced with its
  layers and drivers in connection order.
- Given a scene with no single correct answer -- more than one parent, a
  cycle, or a motion-sampled transform -- when resolution is requested,
  then it fails with a typed error rather than returning a matrix.

### User Story 4: Prove Fidelity Against A Real Renderer (P2)

As a maintainer, I want a recorded scene to replay as the same ɴsɪ
stream a production renderer writes, so that recording fidelity is
demonstrated rather than asserted.

**Acceptance Criteria**

- Given one scene-building function driving both a 3Delight `apistream`
  context and the recorder, when both streams are canonicalised, then
  they are equal, for a scene meeting the preconditions in R10.

## Non-Goals

- Rendering. This surface produces no pixels.
- Any renderer-specific mapping. `Properties`, `SceneObject` and their
  kin belong to a backend spec.
- `evaluate`. Procedurals and Lua imply an execution model this surface
  does not define; it records as a no-op.
- Shader-network resolution. `ShaderNetwork` edges are classified and
  carried with ports intact, because their consumer is OSL, not a graph
  walk.
- Resolving an instanced node to its per-path transforms. Multi-parent
  is detected and rejected, not expanded; see R11.

## Requirements

- R1: A `Recorder` implements all nine `nsi_trait::Nsi` methods.
- R2: `Recorder` is `Send + Sync`, as the trait requires.
- R3: Every argument except `Type::Reference` is copied during the call.
- R4: `Type::Reference` stores the host address, never its contents, and
  is never forwarded to a renderer as an object link.
- R5: Connection classification is exhaustive; an unknown destination
  attribute is an error. A `from_attr` of `Some("")` is `None`. All
  three ɴsɪ shader slots -- `surfaceshader`, `displacementshader`,
  `volumeshader` -- are classified, as are `members`, `lightset` and
  `shaderattributes`: ɴsɪ's documented light-set workflow connects
  lights to a `set` node and that node to an `outputlayer`, and
  rejecting either destination made the whole workflow unrecordable.
- R6: Node and attribute order is insertion order.
- R7: Motion samples are stored separately from static attributes and
  sorted by time. Sample times are keyed by a *total* order, so a `NaN`
  time matches itself and `-0.0` is distinct from `0.0`. The two setters
  replace each other per name, as ɴsɪ requires: "Setting an attribute
  using this function replaces any value previously set by
  `NSISetAttribute` or `NSISetAttributeAtTime`."
- R8: Transform chains compose in row-vector order.
- R9: A malformed scene containing a cycle must not hang the resolver,
  and must not answer it either: it is a typed error.
- R10: A recorded scene replays as an ɴsɪ stream equal to 3Delight's,
  **for a scene that meets the recorder's preconditions**: one attribute
  per call, a node's static attributes set before its motion samples, no
  repeated `create`, and no `delete`, `delete_attribute` or `disconnect`.
  A recorder holds scene state and 3Delight's `apistream` is a call log;
  outside those preconditions the two differ by construction.
- R11: A node with more than one `objects` parent is ɴsɪ's lightweight
  instancing. Resolving a single world transform for one is a typed
  error, not an answer for whichever parent was connected first.
- R12: Attributes are gathered along the whole path, "starting from the
  geometric primitive, through all the transform nodes it is connected
  to, until the scene root is reached" -- `.root` included, since ɴsɪ
  describes it as "much like a transform node" carrying its own
  `geometryattributes`.

  *Every* `attributes` node on that path is kept, because ɴsɪ says so:
  "one attributes node can set object visibility and another can set the
  surface shader ... and will all be considered". They are ordered by
  ɴsɪ's rule -- "the definition with the highest priority is selected.
  In case of conflicting priorities, the definition that is the closest
  to the geometric primitive" -- and connection order breaks a remaining
  tie. Each shader slot resolves by the priority of its own connection,
  which ɴsɪ calls "useful for overriding a shader from higher in the
  scene graph", and agrees with that same order.
- R13: A motion-sampled `transformationmatrix` resolves per sample.
  `motion_times` gives the chain's sample times, `world_transform_at` one
  composed matrix, and `world_transform_samples` the pair. A node with no
  samples is constant and contributes at every time. Nothing is ever
  interpolated: element-wise interpolation of a matrix is wrong for
  anything containing a rotation, so a time between samples is an error
  naming the times that exist. `world_transform` remains the static
  accessor and still refuses a sampled chain.
- R14: A `Callback` argument leaks its payload. `Callback::type_`
  reports `Type::Reference`, so a recorder cannot distinguish one, and
  `Callback::drop_fn` is `pub(crate)` to `nsi-ffi-wrap`, so it could not
  reclaim one. This is an accepted limitation, not an oversight.
- R15: `Recorder::scene` returns a guard over the lock every `Nsi`
  method takes. Calling one while a guard is alive deadlocks. This is
  documented on the method rather than designed away.
- R16: Every `connect` argument is recorded whole, so `"strength"` --
  which blocks a recursive delete -- and `"value"` survive for a backend,
  and replay emits what was passed. The arguments to `create` and
  `delete` are still dropped.
- R17: A node's identity is its handle. Re-`create` with the same type is
  a no-op and with a different type an error, because ɴsɪ says it
  "does nothing if all other parameters match ... Otherwise, it emits an
  error".
- R18: A connection's identity is `(from, from_attr, to, to_attr)`.
  Repeating one updates its arguments rather than recording a second
  edge, because ɴsɪ says "it is not an error to create a connection
  which already exists" -- and a duplicate would read as a second parent.
  Both handles must already exist: "the nodes on which the connection is
  performed must exist". `.root` and `.global` are reserved and need no
  `create`.
- R19: `disconnect` honours `.all` in all four positions -- the *source
  attribute* included, which is what ɴsɪ means by "the handle for either
  node, as well as any or all of the attributes".
- R20: A node not connected to `.root` is not in the scene, and
  resolving one is an error rather than identity: ɴsɪ says such a node
  "won't affect the render in any way". An instancing prototype reaches
  the scene through its `instances` node, so its attributes gather
  normally -- but it has no single world transform, because ɴsɪ gives an
  `instances` node "a transformation matrix for each instance". Asking
  for one is an error, not the instancer's own matrix.
- R23: `.root` and `.global` are reserved. They need no `create`, they
  are never declared in a replayed stream, and deleting one is an error:
  ɴsɪ says "it is not possible to delete the root or the global node",
  and deleting `.root` here would strip every membership edge.
- R24: Instancing prototypes are ordered by the `"index"` argument of
  their connection, which is what an `instances` node's `modelindices`
  selects into -- not by connection order.
- R21: A replayed stream is escaped. A string carrying a quote or a
  newline must not close its literal, because the reader would parse the
  remainder as further statements.
- R22: Doubles replay as C's `%.17g`, which is what 3Delight writes.
  Argument flags replay as the letter prefixes it writes inside the type
  name. A `Reference` argument's parameter line is omitted, its
  statement kept. Exactly one scalar is written bare; everything else is
  bracketed, an empty slice included, which 3Delight writes as `[ ]`.

## Risks

- **Silent connection miscategorisation.** A connection mapped to the
  wrong concept does not error; it renders, with materials on the wrong
  shapes. Mitigated by an exhaustive classifier that rejects unknown
  destinations, and by contract rows requiring per-class evidence.
- **Composition order.** Composing a transform chain backwards produces
  correct output whenever transforms commute, and wrong output
  otherwise. Mitigated by a test using a non-commuting pair.
- **Pointer marshalling drift.** `Reference::as_c_ptr` yields a pointer
  to the pointer. Dereferencing one level too few or too many is
  invisible, because a pointer is opaque either way. Mitigated by a test
  asserting the recorded address equals a known payload's, driven
  through `Nsi::set_attribute` rather than the marshalling alone.
- **Grouping loss.** A recorder holds scene state, not a call log, so
  attribute-to-call grouping is discarded. Stream comparison therefore
  requires the R10 preconditions. Accepted: a renderer only ever sees
  final values.
- **A fixture-shaped fidelity gate.** R10 is proven by one scene. Every
  behaviour it does not exercise -- argument flags, float formatting,
  `Reference` payloads -- is unproven however green the gate is. Named
  per row in `contracts/stream.md` rather than folded into R10.
- **Silently ignored call parameters.** ɴsɪ's `recursive` delete is
  still dropped. R16 makes that a decision; the `Open` row in
  `contracts/recording.md` makes it a tracked one.
- **Reading the wrapper rather than the specification.** Two review
  rounds found this surface inventing semantics ɴsɪ already defines,
  because the `nsi-ffi-wrap` docstrings summarise where `nsi.pdf`
  states. Every requirement above now quotes the specification, and a
  rule that is chosen rather than quoted says so.
- **Non-UTF-8 strings are still lossy.** `to_string_lossy` replaces
  invalid bytes at *recording* time, so the byte is gone before replay
  can escape it. 3Delight round-trips it as `\xE9`. Tracked as an `Open`
  row; it needs byte storage, not an escaping change.
