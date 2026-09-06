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
- `binarynsi`. See R28.
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
- R5: Every `<connection>` attribute the specification declares is
  classified by name. A `from_attr` of `Some("")` is `None`.

  **A destination that is not one of them is carried, not refused.**
  ɴsɪ's set of destinations is open: its own §4.8 connects one
  `attributes` node to another's `visibility` to override a value, and
  `facesets` appears in Listing 3.2. Enumerating them is therefore
  impossible in principle, and refusing what is not listed made legal
  scenes unrecordable -- an exporter using a lens shader or a face set
  stopped on its first call.

  The reason the classifier used to refuse still holds, and is met a
  different way: an unlisted destination becomes `EdgeKind::Other`
  carrying its own name, and **resolution never interprets it**. A
  connection becomes a material, a transform link or an output route
  only when its name says so, so the silent miscategorisation this
  surface exists to prevent is still prevented.

  The cost is real and worth stating: a typo'd destination is now
  carried rather than rejected, so it silently does nothing instead of
  failing loudly. 3Delight accepts it silently too, so this matches the
  renderer rather than being stricter than it -- but a strictness that
  caught real mistakes has been traded for the ability to record legal
  scenes.
- R6: Node and attribute order is insertion order.
- R7: Motion samples are stored separately from static attributes and
  sorted by time. A non-finite sample time is refused, as 3Delight
  refuses one (`E6026`), and `-0.0` is the same sample as `0.0`, because
  the renderer reads a `-0` time as `+0`. An earlier version of this
  requirement asserted the opposite of both. The two setters
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
  and replay emits what was passed. `delete` reads `"recursive"` (R31).
  ɴsɪ defines no `create` arguments, so there are none to drop.
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

- R25: A recorded scene can be written back as an ɴsɪ stream (always),
  as a compressed stream (`gzip` and `zstd` features), and as a Lua
  script (`lua` feature). Compression is a property of the file, not the
  format: a compressed stream decompresses to exactly the plain one.
  **Only gzip is read by the renderer.** `renderdl` reads a `.nsi.gz`
  wherever it reads a `.nsi`; handed a zstd stream it fails with
  `Invalid char`, and a context configured with `streamcompression=
  "zstd"` writes plain text. `zstd` is therefore for consumers of this
  crate, and is documented as such rather than as an ɴsɪ format.
- R26: ɴsɪ's Lua binding is narrower than its C API in three ways, and
  an attribute it cannot express is refused rather than degraded:
  - **Types.** `nsi.TypeDouble`, `nsi.TypeInt64` and a pointer type do
    not exist. Those names are `nil`, and a parameter table whose `type`
    is `nil` is a runtime error; passing the value untyped instead makes
    a double a `float` and a large integer a different number.
  - **Flags.** A parameter table has `name`, `data`, `type` and
    `arraylength` and nothing else, so `per_vertex`, `per_face` and
    `linear_interpolation` cannot be said. A per-vertex normal emitted
    without its flag rebuilds a different surface.
  - **Empty string arrays.** Setting one from Lua aborts the renderer
    with a heap error rather than reporting a problem.
- R27: A typed Lua parameter's `data` is always a table, even for a
  single value. 3Delight reads a bare typed scalar as an empty array.
- R28: `binarynsi` is a non-goal for now. ɴsɪ names it, but the encoding
  is undocumented, and matching it means reading the renderer's bytes
  rather than a specification.

- R32: An argument is an array when ɴsɪ's `IsArray` flag says so, not
  when its length exceeds one. `array_len(1)` is a real one-element
  array and replays as `float[1]`.
- R33: `f32` and `f64` replay through *different* printers, because
  3Delight uses different ones. A double is `%.17g`. A float is written
  as the shorter of decimal and exponent notation with an unpadded
  exponent (`1e5`, `1e-7`), which agrees with the renderer on every
  value the gate drives -- but **is not its algorithm**: 3Delight writes
  `0.33333335` where Rust's shortest round-trip gives `0.33333334`, and
  `2e-45` for the smallest denormal. Such values re-parse to the same
  float, so the difference is textual, and the contract row says which
  values are proven rather than claiming the general case.
- R29: A node inside an instancing prototype has no world transform,
  but it does have one relative to an ancestor, which is the space the
  per-instance matrix applies in. `relative_transform` composes it.
- R30: An `instances` node's `transformationmatrices` are paired with
  the prototype each draws, matching `modelindices` against "the index
  attribute of the model connection" rather than against position.
  A negative model index and any handle in `disabledinstances` are not
  rendered, as ɴsɪ says.
- R31: `delete` honours ɴsɪ's `recursive`, with both documented
  exceptions: a node is spared when it "also has connections which do
  not eventually lead to the specified node", or when "their connection
  to the deleted node was created with a strength greater than 0".

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
- **A gate shaped like its fixture.** Both round-trip gates prove only
  what their scenes contain. Five separate defects -- argument flags in
  Lua, `array_len(1)`, `f32` exponent formatting, empty slices and
  `.global` -- were all invisible while green, and each was found by
  widening the fixture rather than by the suite. Every new emitter rule
  adds a case to the fixture for that reason.
- **Reading the wrapper rather than the specification.** Two review
  rounds found this surface inventing semantics ɴsɪ already defines,
  because the `nsi-ffi-wrap` docstrings summarise where `nsi.pdf`
  states. Every requirement above now quotes the specification, and a
  rule that is chosen rather than quoted says so.
- **Non-UTF-8 strings are still lossy.** `to_string_lossy` replaces
  invalid bytes at *recording* time, so the byte is gone before replay
  can escape it. 3Delight round-trips it as `\xE9`. Tracked as an `Open`
  row; it needs byte storage, not an escaping change.
