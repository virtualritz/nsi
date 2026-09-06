# Research: ɴsɪ Intermediate Representation

## Decisions

### D1: `nsi_ffi_wrap::Arg` is the shared argument currency

Consumers build arguments with the `nsi::point_slice!` family, which
produces a concrete `nsi_ffi_wrap::Arg`. Had the recorder invented its
own `Arg`, every generic consumer would need a parallel macro set.

Upstream already assumes this shape: `FfiApiAdapter` is declared
`for<'a> T: Nsi<Arg<'a> = Arg<'a, 'a>>`. Constructing an `Arg` loads no
renderer; the dynamic loader runs only on `Context::new`.

**Consequence:** `impl ParamValue for Arg` had to land inside
`nsi-ffi-wrap`, because `Arg`'s fields are `pub(crate)`.

### D2: The `where Self: 'call` GAT bound was dropped from `nsi-trait`

`impl<'a> Nsi for Context<'a>` cannot satisfy it. Every `Self::Arg<'_>`
in a method signature demands `Context<'a>: 'call` for a fresh
late-bound `'call` that may outlive `'a`, which is unprovable (E0477).

The bound is only needed by an implementor whose `Arg<'call>` borrows
from `Self`. None does. The alternative was narrowing every such impl to
`'static`, forcing `'static` on consumers for no benefit.

**Rejected:** `impl Nsi for Context<'static>`. Compiles, but the trait
impl would exist only for `'static` contexts.

### D3: `Recorder` has no context lifetime, though `Context` does

`Context` stores no pointers — its `'a` is a `PhantomData` marker and
the renderer holds the data. The recorder *retains* `Reference`
addresses so they survive replay. Retaining them while being
`Send + Sync` is sound only if the pointees outlive every thread that
could see them, so the `Arg` GAT is pinned to `'static` and a lifetime
parameter could only ever be `'static`.

This matches `nsi-ffi-wrap`, where `Reference`, `Callback` and
`ReferenceSlice` are `Send`/`Sync` at `'static` and nowhere else.

### D4: The `Send`/`Sync` assertion sits on `HostPtr`, not `Recorder`

A blanket `unsafe impl Send for Recorder` would silently keep covering
any non-`Send` field added later. The newtype scopes the assertion to
the field that needs it.

### D5: The stream format was read, not inferred

3Delight 2.9.207's own `apistream` output was captured and the emitter
written against it. This corrected four assumptions that would each have
produced a plausible but wrong stream:

- `I64` is `int64`, not `int`.
- `MatrixF64` is `doublematrix`, not `matrix`.
- An array length rides *inside* the type name as `int[2]`, with the
  count divided by it, rather than in a separate field.
- A lone scalar is bare; anything longer is bracketed.

### D6: Resolution lives here, not in each backend

Mitsuba has no transform tree, only a `to_world` per shape; MoonRay
resolves to world space too. Neither has an ɴsɪ `attributes` node.
Both need the same walks, so they happen once.

`geometry_binding` returns the attributes *handle* rather than its
contents: visibility flags are encoded differently by each renderer, and
a common shape for them here would be guesswork.

### D7: `OwnedArg` is `PartialEq` but not `Eq` or `Hash`

`AGENTS.md` asks for `Debug`, `Clone`, `Hash`, `PartialEq` and `Eq` on
public types. `OwnedArg` and `OwnedData` carry `f32`/`f64` payloads, and
ɴsɪ hands them through unchanged: a `NaN` in a point buffer is data, not
an error. `Eq` would be a lie about reflexivity and `Hash` would be
inconsistent with a `PartialEq` that says `NaN != NaN`.

**Rejected:** hashing the bit patterns. It makes `Hash` and `PartialEq`
disagree for `-0.0` and `0.0`, which is worse than not having `Hash`.

`Node` and `Scene` inherit this transitively. `PartialEq` alone is
enough for the one thing tests need -- asserting a whole scene is
unchanged, which is how the `evaluate` no-op is proven.

Sample *times* are a separate question and got the opposite answer: they
are map keys, not payload, so `Scene::set_attribute_at_time` matches
them with `f64::total_cmp`. Under `==` a `NaN` time never matches
itself, so every repeat appends another sample and the vector grows
without bound.

### D8: `priority` is documented, and the first version of this entry
was wrong

An earlier draft recorded that ɴsɪ "does not say whether a higher number
wins, nor how ties break", and marked the row `Partial` on that basis.
That was true of the `nsi-ffi-wrap` docstring and false of the
specification, which states: "the definition with the highest priority
is selected. In case of conflicting priorities, the definition that is
the closest to the geometric primitive (i.e. the furthest from the root)
is selected."

The implemented rule -- priority, then proximity -- happened to match.
The reasoning did not, and a row was `Partial` for a reason that did not
exist.

**And the rule it matched was still the wrong one.** The quoted sentence
governs `ATTR.priority`, set on the node. Applying it to the *connection*
`priority`, as this crate then did, is contradicted by the renderer; see
[D12](#d12-the-connection-priority-on-geometryattributes-is-inert). So
this entry's own lesson -- read the specification, not the wrapper --
has a second half: read the renderer where the specification
contradicts itself. Two sentences of `nsi.pdf` disagree about this
argument, and only 3Delight settles which one ships.

**The lesson is the entry.** This surface is built against a published
specification, and two review rounds found it inventing semantics that
`nsi.pdf` already defines, because the Rust wrapper's docstrings
summarise where the specification states. Read `nsi.pdf` first. Where a
rule here is chosen rather than quoted, the contract row says so.

Connection order remains a tie-break -- the second, since D12 removed
the connection priority from the key -- and is *not* in the
specification, which instead says such nodes "will all be considered" --
which is why `Binding::attributes` is a list rather than a winner.

### D10: The legacy attribute spelling, not the documentation draft

The ɴsɪ documentation draft renames most attributes:
`geometryattributes` to `attributes`, `surfaceshader` to
`shader.surface`, `transformationmatrix` to `matrix`, and -- on an
`instances` node -- `sourcemodels` to `objects`.

This crate keeps the legacy spelling, because that is what the renderer
it is verified against writes: 3Delight 2.9.207 emits `surfaceshader`
and `geometryattributes` in its own `apistream` output, and the stream
gate compares against that.

**The rename that matters is `sourcemodels` to `objects`.** Under the
draft, `objects` would mean scene membership on a transform and an
instancing source on an `instances` node, so classification could no
longer depend on the destination attribute alone -- and
`contracts/classification.md`'s invariant "never on node types" would
have to go. The other renames fail loudly, because `classify` rejects an
unknown destination. That one fails silently: the walk would treat an
`instances` node as a parent transform.

**Rejected:** supporting both spellings now. Two vocabularies with no
renderer to test the second against is a guess wearing a compatibility
shim.

### D9: Refusing beats a plausible wrong matrix

Several scenes have no single world transform: more than one `objects`
parent, a cycle, a node that never reaches `.root`, and -- before the
motion API -- a sampled `transformationmatrix`. Each was previously
answered: with the first parent's chain, with whatever composed before a
budget ran out, with identity, and with the static pose.

All three are silent. A wrong matrix renders. The blueprint forbids
silent fallback on required data, so all three became `ResolveError`
variants.

The cost is real: a motion-blurred scene now cannot be resolved at all,
where before it resolved wrongly. That is the right trade only because
the failure is loud and the row that fixes it is `Open` and named.

**Rejected:** `Option<[f64; 16]>`. Three different failures collapsing
into `None` tells a caller nothing about which.

## Rejected Alternatives

- **`OwnedData::Reference(Vec<usize>)`** to sidestep auto-trait issues.
  Loses provenance and needs laundering on replay.
- **A shared `Visibility` type.** Speculative; see D6.
- **Byte-exact stream comparison.** 3Delight wraps long values at an
  arbitrary width, so canonicalisation is required regardless.

## References

- `nsi-trait/src/nsi_trait.rs` — the `Nsi` trait and its GAT.
- `nsi-ffi-wrap/src/argument.rs` — `Arg`, `ArgData`, `to_c_param_vec`,
  `Callback` and its `pub(crate)` `drop_fn`.
- `nsi-ffi-wrap/src/context.rs` — `Context::connect`, which documents
  `priority`, `value` and `strength`, and `Context::disconnect`, which
  documents the `.all` wildcard.
- 3Delight 2.9.207 linux64 "Re-Animator" — the stream oracle.

### D12: the connection `priority` on `geometryattributes` is inert

ɴsɪ documents `connect`'s `priority` twice, and the two do not agree.
The attribute entry is unconditional:

> `priority` ... When connecting attributes nodes, indicates in which
> order the nodes should be considered when evaluating the value of an
> attribute.

The prose in the `attributes` node section hedges:

> Connections **(for shaders, essentially)** can also be assigned
> priorities, which are used in the same way as for regular attributes.

This crate implemented the first reading from round 2 onward and sorted
the gathered nodes by it. 3Delight 2.9 implements the second. The
decisive scene, rendered to a 4x4 EXR and read at the alpha channel:

```
Create "xf" "transform"
Connect "xf" "" ".root" "objects"
Create "mesh" "mesh"
Connect "mesh" "" "xf" "objects"
Create "near" "attributes"
Create "far" "attributes"
Connect "near" "" "mesh" "geometryattributes"
Connect "far" "" "xf" "geometryattributes" "priority" "int" 1 [ 10 ]
SetAttribute "near" "visibility" "int" 1 [ 0 ]
SetAttribute "far" "visibility" "int" 1 [ 1 ]
```

Alpha is **0**: `near` wins and the priority does nothing. Replacing
that connection argument with `"visibility.priority" "int" 1 [ 10 ]`
set on `far` itself gives alpha **1**, so the scene can express "far
wins" -- just not that way. `renderdl -cat` echoes the connection
argument back, so it was parsed and then ignored, not dropped.

Six scenes agree, covering both levels and both directions, same-depth
siblings, and shader resolution. A priority on a `surfaceshader`
connection *is* honoured, which is why `shader_on` still reads one and
`gathered_attributes` does not.

The renderer is the oracle here. A backend that trusted the
specification would place an override on the wrong node and get a
silently different picture from 3Delight, which is worse than not
supporting the argument at all.
