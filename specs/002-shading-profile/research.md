# Research: Shading Profile

Inherits decision D7 from `specs/001-gpu-pixel-streaming/research.md`
(network translation over ᴏsʟ execution; rejected NVIDIA-only OptiX reuse,
ᴏsʟ→SPIR-V backend, CPU-only shading).

## D1: Dual-Implementation Nodes (ᴏsʟ Reference + GPU Codegen)

**Decision.** Every profile node has two implementations of record: an ᴏsʟ
reference (executed by offline renderers -- this is what 3Delight sees) and
a GPU emitter. Parity between them is the product (US2), enforced by an
image-comparison harness.

**Why.** This is MaterialX's proven model (nodedefs with per-target
implementations) and it keeps one ɴsɪ scene renderable on both paths with
no scene-side branching.

**Rejected.** GPU-only node definitions -- forfeits offline parity and the
existing 3Delight validation path.

## D2: Closures As The Integrator Boundary

**Decision.** The profile vocabulary bottoms out in closures, mirroring
ᴏsʟ's architecture: networks compute closure weights/parameters; the
integrator owns sampling. `emission()` is a first-class profile closure
because ɴsɪ has no light nodes -- lights are emissive geometry.

**Rejected.** A monolithic "uber material" parameter struct as the only
surface -- simpler to start but bakes one material model into the contract
and cannot express ɴsɪ's emissive-geometry lighting idiomatically.

## D3: MaterialX Alignment, Pending R3

Aligning the node set with a MaterialX stdlib subset buys interchange (US5)
and possibly its shadergen. Open question (R3 in `spec.md`): reuse
MaterialX nodedefs/shadergen directly (C++ dependency, GLSL→SPIR-V
toolchain) vs. ɴsɪ-native definitions in Rust with a mapping layer
(cleaner workspace fit, more emitter work). Evidence to gather before
deciding: whether MaterialX shadergen output is usable under Vulkan without
per-node patching, and the dependency cost in this pure-Rust workspace.

## D4: Deterministic Parameter Blocks

Translated networks must yield stable, documented parameter-block layouts
(R6) so engines can animate material parameters per frame without
re-translation -- the same edit-with-attributes philosophy ɴsɪ applies to
scene data, applied to materials. Re-translation happens only on topology
edits (connect/disconnect), mirroring how `synchronize` distinguishes
attribute edits from structural edits.

## References

- D7 decision record: `specs/001-gpu-pixel-streaming/research.md`.
- ᴏsʟ GPU status (OptiX-only upstream backend):
  <https://github.com/AcademySoftwareFoundation/OpenShadingLanguage>.
- MaterialX shadergen: <https://materialx.org/>.
- Isotropix Angie precedent (ᴏsʟ + MaterialX, hybrid CPU/GPU):
  <https://www.fxguide.com/quicktakes/angie-hybrid-renderer-from-isotropix/>.
- ɴsɪ lighting model (emissive geometry):
  <https://nsi.readthedocs.io/en/latest/guidelines.html>.
- Existing image-comparison test machinery: `AGENTS.md`
  (`RUST_TEST_UPDATE=1`).
