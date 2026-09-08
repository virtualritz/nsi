//! The control cage, read out of an ɴsɪ `mesh`.
//!
//! ɴsɪ and [`subdiv_kernels`] describe the same cage differently, and
//! the gap is small but not empty. ɴsɪ gives face-vertex counts, an
//! optional `P.indices`, and creases as *vertex pairs*; the kernel
//! wants a canonical edge list with one crease value per edge. So the
//! edges are built here, from the faces, and the creases mapped onto
//! them.

use crate::Error;
use nsi_intermediate::Scene;
use std::collections::HashMap;
use subdiv_kernels::topology::Mesh;

/// An ɴsɪ subdivision cage, in the kernel's terms.
#[derive(Debug, Clone)]
pub struct Cage {
    /// The topology the kernel refines.
    pub mesh: Mesh,
    /// The scheme ɴsɪ asked for.
    pub scheme: subdiv_kernels::Scheme,
}

impl Cage {
    /// Read one from a `mesh` node.
    ///
    /// # Errors
    ///
    /// [`Error::NotASubdivisionSurface`] for a mesh with no
    /// `subdivision.scheme`, [`Error::UnknownScheme`] for one this
    /// crate does not implement, [`Error::HolesInCage`] for a cage with
    /// `nholes`, and [`Error::Resolve`] when the faces do not resolve.
    pub fn read(scene: &Scene, handle: &str) -> Result<Self, Error> {
        let node = scene.node(handle).ok_or_else(|| Error::UnknownHandle {
            handle: handle.to_string(),
        })?;

        let scheme = match node.string(SCHEME) {
            None => {
                return Err(Error::NotASubdivisionSurface {
                    handle: handle.to_string(),
                });
            }
            // ɴsɪ documents exactly one value: "a value of
            // \"catmull-clark\" will cause the mesh to render as a
            // Catmull-Clark subdivision surface".
            Some("catmull-clark") => subdiv_kernels::Scheme::CatmullClark,
            Some(other) => {
                return Err(Error::UnknownScheme {
                    handle: handle.to_string(),
                    scheme: other.to_string(),
                });
            }
        };

        let faces: Vec<_> = scene.faces(handle)?.collect();
        if faces.iter().any(|face| !face.holes.is_empty()) {
            return Err(Error::HolesInCage {
                handle: handle.to_string(),
            });
        }

        let face_vertex_counts: Vec<u32> =
            faces.iter().map(|face| face.outer as u32).collect();
        let corners: usize =
            face_vertex_counts.iter().map(|count| *count as usize).sum();

        // Without `P.indices` a mesh lists its vertices in face order,
        // so the indices are the positions themselves.
        let face_vertex_indices: Vec<u32> = node
            .effective(POSITION_INDICES)
            .and_then(|argument| argument.as_i32s())
            .map_or_else(
                || (0..corners as u32).collect(),
                |indices| indices.iter().map(|index| *index as u32).collect(),
            );

        let vertex_count = face_vertex_indices
            .iter()
            .max()
            .map_or(0, |highest| highest + 1)
            .max(
                node.effective(POSITIONS)
                    .map_or(0, |argument| argument.element_count() as u32),
            );

        let (edge_vertices, edge_of) =
            edges(&face_vertex_counts, &face_vertex_indices);
        let edge_creases = creases(node, &edge_of, edge_vertices.len());
        let vertex_corners = vertex_corners(node, vertex_count as usize);

        Ok(Self {
            mesh: Mesh {
                vertex_count,
                face_vertex_counts,
                face_vertex_indices,
                edge_vertices,
                edge_creases,
                vertex_corners,
            },
            scheme,
        })
    }
}

/// ɴsɪ mesh attributes this crate reads.
const SCHEME: &str = "subdivision.scheme";
const POSITIONS: &str = "P";
const POSITION_INDICES: &str = "P.indices";
const CREASE_VERTICES: &str = "subdivision.creasevertices";
const CREASE_SHARPNESS: &str = "subdivision.creasesharpness";
const CORNER_VERTICES: &str = "subdivision.cornervertices";
const CORNER_SHARPNESS: &str = "subdivision.cornersharpness";

/// The canonical undirected edges of a cage, and where to find one.
///
/// ɴsɪ never states the edges: they are implied by the faces, and a
/// crease names its edge by the two vertices at its ends. So both the
/// list and the lookup are built here, once.
fn edges(
    face_vertex_counts: &[u32],
    face_vertex_indices: &[u32],
) -> (Vec<[u32; 2]>, HashMap<[u32; 2], usize>) {
    let mut edge_vertices = Vec::new();
    let mut edge_of = HashMap::new();
    let mut corner = 0usize;

    for count in face_vertex_counts {
        let count = *count as usize;
        for position in 0..count {
            let from = face_vertex_indices[corner + position];
            let to = face_vertex_indices[corner + (position + 1) % count];
            let key = canonical(from, to);
            edge_of.entry(key).or_insert_with(|| {
                edge_vertices.push(key);
                edge_vertices.len() - 1
            });
        }
        corner += count;
    }

    (edge_vertices, edge_of)
}

/// An edge is the same edge whichever way a face walks it.
fn canonical(from: u32, to: u32) -> [u32; 2] {
    if from <= to { [from, to] } else { [to, from] }
}

/// ɴsɪ's creases, one value per edge of the cage.
///
/// `subdivision.creasevertices` is "a list of crease edges. Each edge
/// is specified as a pair of indices into the `P` attribute", and
/// `subdivision.creasesharpness` has "a value for each pair". A pair
/// naming an edge the faces do not have is ignored rather than
/// refused: it creases nothing.
fn creases(
    node: &nsi_intermediate::Node,
    edge_of: &HashMap<[u32; 2], usize>,
    edges: usize,
) -> Vec<f32> {
    let mut creases = vec![0.0; edges];
    let (Some(pairs), Some(sharpness)) = (
        node.effective(CREASE_VERTICES).and_then(|a| a.as_i32s()),
        node.effective(CREASE_SHARPNESS).and_then(|a| a.as_f32s()),
    ) else {
        return creases;
    };

    for (pair, sharpness) in pairs.as_chunks::<2>().0.iter().zip(sharpness) {
        let key = canonical(pair[0] as u32, pair[1] as u32);
        if let Some(edge) = edge_of.get(&key) {
            creases[*edge] = *sharpness;
        }
    }

    creases
}

/// ɴsɪ's sharp corners, one value per vertex.
fn vertex_corners(node: &nsi_intermediate::Node, vertices: usize) -> Vec<f32> {
    let mut corners = vec![0.0; vertices];
    let (Some(indices), Some(sharpness)) = (
        node.effective(CORNER_VERTICES).and_then(|a| a.as_i32s()),
        node.effective(CORNER_SHARPNESS).and_then(|a| a.as_f32s()),
    ) else {
        return corners;
    };

    for (vertex, sharpness) in indices.iter().zip(sharpness) {
        if let Some(corner) = corners.get_mut(*vertex as usize) {
            *corner = *sharpness;
        }
    }

    corners
}
