# `nsi-tessellate`

Explicit geometry from ɴsɪ subdivision surfaces, for consumers that
cannot intersect them analytically.

3Delight ray-traces the limit surface and never tessellates unless
displacement forces it; a rasteriser, a ɢᴘᴜ ʙᴠʜ builder or a tool
dumping a mesh to print cannot. This crate maps ɴsɪ's description onto
[`subdiv-kernels`](https://crates.io/crates/subdiv-kernels) and
implements no subdivision of its own.

**Optional, not a stage.** An analytic renderer goes straight from
`nsi-intermediate` to its own scene. See `specs/007-tessellation`.
