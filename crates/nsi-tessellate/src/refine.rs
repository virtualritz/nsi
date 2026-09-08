//! Refining a cage, and carrying its primitive variables along.

use crate::{Cage, Error};
use core::num::NonZeroU8;
use nsi_intermediate::{Interpolation, Scene};
use subdiv_kernels::{
    FaceVaryingInterpolation, Refiner, SchemeOptions, UniformRefine,
    topology::FaceVaryingChannel,
};

/// How finely to refine.
///
/// The kernel refines by **level**, and a level doubles the face count
/// each step. A caller with a tolerance -- a maximum edge length, a
/// screen-space error -- converts it once with [`Options::for_edge_length`]
/// rather than each backend inventing its own mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// How many uniform refinement levels.
    pub levels: NonZeroU8,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            // SAFETY: 2 is non-zero.
            levels: NonZeroU8::new(2).expect("2 is non-zero"),
        }
    }
}

impl Options {
    /// The levels that bring a cage's longest edge under `target`.
    ///
    /// Each Catmull-Clark step halves an edge, so this is
    /// `log2(longest / target)`, rounded up and clamped to at least one
    /// level and at most `limit`. **No view is involved**: a consumer
    /// dumping a mesh to print wants a tolerance in scene units, and a
    /// consumer with a camera converts screen-space error into one
    /// before calling.
    #[must_use]
    pub fn for_edge_length(longest: f64, target: f64, limit: u8) -> Self {
        let steps = if target <= 0.0 || longest <= target {
            1
        } else {
            (longest / target).log2().ceil().max(1.0) as u32
        };
        let levels = steps.min(u32::from(limit.max(1))) as u8;
        Self {
            // SAFETY: `steps` is at least 1 and `limit` is forced to at
            // least 1, so the minimum of the two cannot be zero.
            levels: NonZeroU8::new(levels).expect("at least one level"),
        }
    }
}

/// A refined subdivision surface.
#[derive(Debug, Clone)]
pub struct Tessellation {
    positions: Vec<[f32; 3]>,
    face_vertex_counts: Vec<u32>,
    face_vertex_indices: Vec<u32>,
}

impl Tessellation {
    /// The refined positions.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// How many corners each refined face has. Every one is a quad
    /// under Catmull-Clark, whatever the cage was.
    #[must_use]
    pub fn face_vertex_counts(&self) -> &[u32] {
        &self.face_vertex_counts
    }

    /// The refined faces, as indices into [`Tessellation::positions`].
    #[must_use]
    pub fn face_vertex_indices(&self) -> &[u32] {
        &self.face_vertex_indices
    }
}

/// Refine an ɴsɪ subdivision surface into explicit geometry.
///
/// # Errors
///
/// Everything [`Cage::read`] returns, plus [`Error::Kernel`] when the
/// cage is one the kernel refuses -- a non-manifold one, say.
pub fn subdivision_surface(
    scene: &Scene,
    handle: &str,
    options: &Options,
) -> Result<Tessellation, Error> {
    let cage = Cage::read(scene, handle)?;
    let positions = positions(scene, handle);

    let refiner =
        Refiner::new(cage.mesh, cage.scheme, SchemeOptions::default())?;
    let request = UniformRefine::from(options.levels);
    let result = refiner.refine_uniform(&request)?;

    Ok(Tessellation {
        positions: result.interpolate(&positions),
        face_vertex_counts: result.topology.face_vertex_counts.clone(),
        face_vertex_indices: result.topology.face_vertex_indices.clone(),
    })
}

/// Refine one primitive variable through the same refinement.
///
/// The class comes from `nsi-intermediate`, which resolves what ɴsɪ
/// leaves implicit; this only routes it. A face-varying variable goes
/// through the kernel's face-varying stencils, a per-vertex one
/// through the vertex stencils like positions, and a uniform or
/// constant one is carried as it is -- there is nothing to interpolate.
///
/// # Errors
///
/// As [`subdivision_surface`], plus whatever resolving the variable
/// returns -- notably an ambiguous interpolation, which this crate
/// will not guess at any more than `nsi-intermediate` will.
pub fn refine_variable(
    scene: &Scene,
    handle: &str,
    name: &str,
    options: &Options,
) -> Result<Option<Vec<[f32; 3]>>, Error> {
    let Some(variable) = scene.primitive_variable(handle, name)? else {
        return Ok(None);
    };
    let Some(values) = variable.values().as_f32s() else {
        return Ok(None);
    };
    let triples: Vec<[f32; 3]> = values.as_chunks::<3>().0.to_vec();

    let cage = Cage::read(scene, handle)?;
    let refiner =
        Refiner::new(cage.mesh, cage.scheme, SchemeOptions::default())?;
    let request = UniformRefine::from(options.levels);

    match variable.interpolation() {
        // Nothing to refine: one value for the whole primitive, or one
        // per cage face that every face it splits into inherits.
        Interpolation::Constant | Interpolation::Uniform => Ok(Some(triples)),
        Interpolation::Vertex => {
            let result = refiner.refine_uniform(&request)?;
            Ok(Some(result.interpolate(&triples)))
        }
        Interpolation::FaceVarying => {
            let channel = FaceVaryingChannel {
                indices: variable
                    .face_varying_indices()
                    .map(|index| index as u32)
                    .collect(),
                value_count: triples.len() as u32,
            };
            let tables = refiner.face_varying_stencils(
                &request,
                &channel,
                FaceVaryingInterpolation::SmoothWithLinearBoundaries,
            )?;
            Ok(Some(
                tables
                    .iter()
                    .fold(triples, |data, table| table.interpolate(&data)),
            ))
        }
    }
}

/// The cage's positions, as the kernel wants them.
fn positions(scene: &Scene, handle: &str) -> Vec<[f32; 3]> {
    scene
        .node(handle)
        .and_then(|node| node.effective("P"))
        .and_then(|argument| argument.as_f32s())
        .map(|values| values.as_chunks::<3>().0.to_vec())
        .unwrap_or_default()
}
