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

### D8: `priority` beats proximity, and the direction is unverified

ɴsɪ documents `connect`'s `"priority"` as indicating "in which order the
nodes should be considered when evaluating the value of an attribute".
It does not say whether a higher number wins, nor how ties break.

`geometry_binding` implements: highest priority, then nearest the
geometry, then connection order. Higher-wins matches the ordinary
reading of the word and 3Delight's behaviour as understood, but it has
**not** been observed against the renderer, so the row in
`contracts/resolution.md` is `Partial` and says so.

The mechanism -- recording `priority` on the edge, ordering candidates
by it -- is right whichever way the comparison points. Only the
comparison would change.

### D9: Refusing beats a plausible wrong matrix

Three scenes have no single world transform: more than one `objects`
parent, a cycle, and a motion-sampled `transformationmatrix`. Each was
previously answered -- with the first parent's chain, with whatever
composed before a budget ran out, and with the static pose.

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
