//! Explicit geometry from ɴsɪ subdivision surfaces.
//!
//! ɴsɪ describes geometry a renderer may not be able to intersect
//! directly. A `mesh` with `subdivision.scheme` is a *control cage* for
//! a limit surface, not the surface itself. 3Delight intersects that
//! surface analytically and never tessellates unless a displacement
//! shader forces it -- but a rasteriser, a ɢᴘᴜ ʙᴠʜ builder, a realtime
//! backend or a tool dumping a mesh to print all need triangles, and
//! each of them writes this conversion itself today.
//!
//! # What this crate is, and is not
//!
//! It **maps** ɴsɪ onto [`subdiv_kernels`] and implements no
//! subdivision of its own. That crate holds no geometry: it returns
//! stencil tables -- sparse maps where each output point is a weighted
//! sum of a few inputs -- which are then applied to positions, `st`,
//! `N` or any other channel. So one refinement carries every primitive
//! variable, and this crate owns no mesh type.
//!
//! It is **optional**. An analytic renderer goes straight from
//! `nsi-intermediate` to its own scene; if it needs this crate, the
//! layering is wrong.
//!
//! A polygon `mesh` **without** `subdivision.scheme` is not touched:
//! that mesh *is* the geometry, and refining it would hand a consumer
//! a different shape than the scene described.
//!
//! # Example
//!
//! ```no_run
//! # use nsi_intermediate::Scene;
//! # use nsi_tessellate::{Options, subdivision_surface};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let scene = Scene::default();
//! let refined = subdivision_surface(&scene, "cage", &Options::default())?;
//! for face in refined.face_vertex_counts() {
//!     // Every face is a quad under Catmull-Clark.
//! }
//! # Ok(())
//! # }
//! ```

mod cage;
mod error;
mod refine;

pub use cage::Cage;
pub use error::Error;
pub use refine::{Options, Tessellation, refine_variable, subdivision_surface};
