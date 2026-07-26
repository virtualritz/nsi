# Contract: Stream Attribute Vocabulary

## Scope

This contract covers parsing, validation, and negotiation of the
`stream.*` attribute vocabulary on the `outputdriver` node (see
`../data-model.md` for the table of record). It does not cover publication
timing or image lifecycle (see `publication-lifecycle.md`).

## Matrix

| Behavior | Status | Source Evidence | Test/QA Evidence | Required Next Evidence |
| --- | --- | --- | --- | --- |
| Driver is addressed via `drivername "nsi-stream"`; all `stream.*` attributes reach it unmodified | Covered | `crates/nsi-stream/src/config.rs` (`StreamConfig::parse`, `Attr`/`AttrValue`) | `cargo test -p nsi-stream vocabulary_forwarding` -- ok (2026-07-26; parser fixture -- bridge/mock-driver forwarding re-check pending T16) | `cargo test -p nsi-stream vocabulary_forwarding` against the bridge or a mock driver. |
| Missing `stream.version` or unsupported version fails open() with a typed error | Covered | `crates/nsi-stream/src/config.rs` + `src/error.rs` (`Error::MissingAttribute`, `Error::UnsupportedVersion`) | `cargo test -p nsi-stream vocabulary_version_reject` -- ok (2026-07-26) | `cargo test -p nsi-stream vocabulary_version_reject`. |
| Unknown `stream.*` attribute is ignored with a warning, not an error | Covered | `crates/nsi-stream/src/config.rs` (`Warning`) | `cargo test -p nsi-stream vocabulary_unknown_attr_warns` -- ok (2026-07-26) | `cargo test -p nsi-stream vocabulary_unknown_attr_warns`. |
| `stream.transport "auto"` negotiates gpu → shm → callback and reports the selected transport | Covered | `crates/nsi-stream/src/transport/mod.rs` (`negotiate`, `TransportProbe`, `StaticProbe`) | `cargo test -p nsi-stream transport_auto_negotiation` -- ok (2026-07-26, forced-unviable fixtures) | `cargo test -p nsi-stream transport_auto_negotiation` with forced-unviable fixtures. |
| Explicit transport that is unviable fails open() with a typed error, no fallback | Covered | `crates/nsi-stream/src/transport/mod.rs` (`Error::TransportUnavailable`, no fallback path) | `cargo test -p nsi-stream transport_explicit_no_fallback` -- ok (2026-07-26) | `cargo test -p nsi-stream transport_explicit_no_fallback`. |
| `stream.device.uuid` mismatch fails/falls back per transport rules | Covered | `crates/nsi-stream/src/transport/mod.rs` (`Error::DeviceMismatch` explicit; auto falls through) | `cargo test -p nsi-stream transport_device_mismatch` -- ok (2026-07-26) | `cargo test -p nsi-stream transport_device_mismatch`. |
| Per-layer format from each connected `outputlayer` is honored (RGBA f16/f32 minimum) | Partial | `crates/nsi-stream/src/layer.rs` (`LayerFormat`, `Layer`), `src/ring.rs` (per-layer planes) | `cargo test -p nsi-stream layer_formats` -- ok (2026-07-26); multi-AOV manual QA pending viewport example (T7) + bridge (T16) | `cargo test -p nsi-stream layer_formats`; manual QA: multi-AOV example shows beauty + ID layer. |

## Invariants

- The contract is expressible through standard `NSICreate`/`NSISetAttribute`
  /`NSIConnect` only; no new API calls (R1).
- Client → renderer flow uses attributes only; the driver initiates every
  reverse-direction message (D3, data-model "Direction").
- Pointer-typed attributes appear only in the in-process callback transport
  (R2).

## Failure Modes

- Unsupported version → typed `Error::UnsupportedVersion` at open; render
  aborts the output chain for this driver only.
- Unviable explicit transport → typed `Error::TransportUnavailable`; no
  partial allocation (state machine: Configured → Failed).
- Malformed handle/channel name → typed error naming the attribute.

## Required Evidence Before Marking Complete

- Source evidence must cite `crates/nsi-stream/src/` symbols implementing
  each row (parser, negotiation, error types).
- Executable evidence: the exact `cargo test -p nsi-stream <test_name>`
  commands listed per row, run without `--release`.
- Manual QA evidence (layer-format row): command line of the multi-AOV
  example, environment (renderer + `DELIGHT` version if bridge), and the
  observed result.
