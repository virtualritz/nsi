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

### User Story 2: Know What A Connection Means (P1)

As a backend author, I want each ɴsɪ connection classified by its
meaning, so that I do not map an output-routing edge onto a material
reference.

**Acceptance Criteria**

- Given a connection, when it is recorded, then it is classified by its
  destination attribute into exactly one known class.
- Given a destination attribute with no mapping, when it is recorded,
  then the call fails rather than defaulting to a guess.

### User Story 3: Receive Resolved Facts, Not Graph Semantics (P1)

As a backend author, I want ɴsɪ's graph semantics applied for me, so
that every backend does not re-derive transform composition and
material binding.

**Acceptance Criteria**

- Given a chain of transform nodes, when a leaf's world transform is
  requested, then the chain is composed in ɴsɪ's row-vector order.
- Given `shader -> attributes -> geometry`, when a geometry's binding is
  requested, then the shader is resolved through the intermediate node.
- Given `outputdriver -> outputlayer -> screen -> camera`, when render
  outputs are requested, then one entry per screen is produced with its
  layers and drivers in connection order.

### User Story 4: Prove Fidelity Against A Real Renderer (P2)

As a maintainer, I want a recorded scene to replay as the same ɴsɪ
stream a production renderer writes, so that recording fidelity is
demonstrated rather than asserted.

**Acceptance Criteria**

- Given one scene-building function driving both a 3Delight `apistream`
  context and the recorder, when both streams are canonicalised, then
  they are equal.

## Non-Goals

- Rendering. This surface produces no pixels.
- Any renderer-specific mapping. `Properties`, `SceneObject` and their
  kin belong to a backend spec.
- `evaluate`. Procedurals and Lua imply an execution model this surface
  does not define; it records as a no-op.
- Shader-network resolution. `ShaderNetwork` edges are classified and
  carried with ports intact, because their consumer is OSL, not a graph
  walk.

## Requirements

- R1: A `Recorder` implements all nine `nsi_trait::Nsi` methods.
- R2: `Recorder` is `Send + Sync`, as the trait requires.
- R3: Every argument except `Type::Reference` is copied during the call.
- R4: `Type::Reference` stores the host address, never its contents, and
  is never forwarded to a renderer as an object link.
- R5: Connection classification is exhaustive; an unknown destination
  attribute is an error.
- R6: Node and attribute order is insertion order.
- R7: Motion samples are stored separately from static attributes and
  sorted by time.
- R8: Transform chains compose in row-vector order.
- R9: A malformed scene containing a cycle must not hang the resolver.
- R10: A recorded scene replays as an ɴsɪ stream equal to 3Delight's.

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
  asserting the recorded address equals a known payload's.
- **Grouping loss.** A recorder holds scene state, not a call log, so
  attribute-to-call grouping is discarded. Stream comparison therefore
  requires one attribute per call. Accepted: a renderer only ever sees
  final values.
