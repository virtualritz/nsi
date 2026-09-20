//! ɴsɪ `nurbs` nodes, tessellated so that declared shared boundaries stay
//! watertight.
//!
//! Two surfaces that meet at an edge each approximate that edge from their
//! own side, and the two approximations differ: a crack, which a
//! displacement opens further. More triangles do not close it. What closes
//! it is knowing that the two boundaries are *the same edge*, which is what
//! ɴsɪ's [weld declarations] say and what `nsi-intermediate` resolves
//! (`Scene::welds`).
//!
//! So the `nurbs` nodes of one weld namespace are meshed as one
//! `monstertruck` shell, in which every `(weld, id)` group is a single
//! shared edge. The mesher samples a shared edge once and both faces use
//! those samples; the samples are then merged, and normals computed on the
//! merged surface, so both sides of a seam carry the same position and the
//! same normal.
//!
//! Vertex identity at the ends of edges comes from topology, never from
//! positions: consecutive boundary pieces of a trim loop share their
//! junction, and a shared edge identifies the junctions at its ends on
//! every face that uses it.
//!
//! Direction comes from the declaration too. Every use of a weld follows
//! one reference traversal once its ranges, segment order and
//! `weld.reverse` are applied, so two uses run the same way along their
//! shared edge exactly when they sit the same way against that
//! traversal. A closed boundary's ends are one point and cannot tell the
//! two senses apart, which is why the contract has the exporter declare
//! the direction and forbids a renderer from measuring it.
//!
//! [weld declarations]: https://nsi.readthedocs.io/en/latest/design/shared-boundaries.html

mod patch;
mod shell;

pub use shell::{NurbsMesh, NurbsOptions, NurbsTessellation, nurbs_meshes};
