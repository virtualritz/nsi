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

## Rejected Alternatives

- **`OwnedData::Reference(Vec<usize>)`** to sidestep auto-trait issues.
  Loses provenance and needs laundering on replay.
- **A shared `Visibility` type.** Speculative; see D6.
- **Byte-exact stream comparison.** 3Delight wraps long values at an
  arbitrary width, so canonicalisation is required regardless.

## References

- `nsi-trait/src/nsi_trait.rs` — the `Nsi` trait and its GAT.
- `nsi-ffi-wrap/src/argument.rs` — `Arg`, `ArgData`, `to_c_param_vec`.
- 3Delight 2.9.207 linux64 "Re-Animator" — the stream oracle.
