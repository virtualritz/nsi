//! Patches of one weld namespace, meshed as one shell.

use super::patch::{self, Patch, Surface, TrimCurve};
use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use monstertruck::{
    geometry::prelude::{ParameterCurve, Point3},
    meshing::prelude::{
        PolygonMesh, TessellationOptions, trimmed_cshell_triangulation_with,
    },
    topology::compress::{
        CompressedEdge, CompressedEdgeUse, CompressedTrimmedFace,
        CompressedTrimmedShell,
    },
    traits::{BoundedCurve, Concat, Invertible, ParametricCurve},
};
use nsi_intermediate::{Scene, WeldKind};

/// A shared edge's curve, and a face-local trim: a trim curve on the
/// surface it lies on.
type EdgeCurve = ParameterCurve<TrimCurve, Surface>;

/// How finely to tessellate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NurbsOptions {
    /// The largest distance, in scene units, between the surface and its
    /// triangles.
    pub tolerance: f64,
}

impl Default for NurbsOptions {
    fn default() -> Self {
        Self { tolerance: 0.01 }
    }
}

/// One `nurbs` node, tessellated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NurbsMesh {
    /// The node.
    pub geometry: String,
    /// Vertex positions. Along a welded edge they are bit for bit those of
    /// the neighbouring face.
    pub positions: Vec<[f64; 3]>,
    /// Unit vertex normals, computed on the surface merged across welds, so
    /// a seam's two sides agree. They point to 3Delight's front side.
    pub normals: Vec<[f64; 3]>,
    /// Triangles, counter-clockwise seen from the front.
    pub triangles: Vec<[u32; 3]>,
}

/// Every `nurbs` node in a scene, tessellated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NurbsTessellation {
    /// One mesh per node that could be read.
    pub meshes: Vec<NurbsMesh>,
    /// Triangle edges used by one triangle only, over the surface merged
    /// across welds. Zero for a closed solid whose every edge is welded.
    pub open_edges: usize,
    /// Nodes skipped, and weld declarations not honoured, with why.
    pub problems: Vec<String>,
}

/// A patch with the node it came from.
type Named<'a> = (&'a str, Patch);

/// One face over the merged surface: its node, its local-to-global vertex
/// map, and its triangles in global vertex ids.
type FaceTriangles<'a> = (&'a str, Vec<usize>, Vec<[usize; 3]>);

/// A piece of one trim loop: a run of consecutive curves that is one
/// edge use.
struct Piece {
    curve: TrimCurve,
    /// The `(weld node, id)` it is a use of, if it is welded.
    weld: Option<(String, u32)>,
}

/// Tessellates every `nurbs` node in `scene`, welding what its weld
/// declarations join.
pub fn nurbs_meshes(
    scene: &Scene,
    options: &NurbsOptions,
) -> NurbsTessellation {
    let mut tessellation = NurbsTessellation::default();

    // Weld tables first, so a problem in one is reported whatever else
    // happens.
    let welds = scene.welds();
    tessellation
        .problems
        .extend(welds.problems.iter().map(ToString::to_string));

    // Which weld use covers which trim curve, or which whole trim loop,
    // of which node.
    let mut welded = Welded::default();
    let mut namespace_of: HashMap<&str, &str> = HashMap::default();
    for weld in &welds.welds {
        for weld_use in &weld.uses {
            namespace_of.insert(weld_use.geometry, weld.weld);
            let key = (weld.weld.to_string(), weld.id);
            for segment in &weld_use.segments {
                match segment.kind {
                    WeldKind::TrimCurve { curve_index } => {
                        welded
                            .curves
                            .insert((weld_use.geometry, curve_index), key.clone());
                    }
                    WeldKind::TrimLoop { loop_index } => {
                        welded
                            .loops
                            .insert((weld_use.geometry, loop_index), key.clone());
                    }
                    other => tessellation.problems.push(format!(
                        "ɴsɪ weld {:?} {} on {:?}: {other:?} is not tessellated \
                         welded yet; that boundary stays unwelded",
                        weld.weld, weld.id, weld_use.geometry
                    )),
                }
            }
        }
    }

    // Read every patch, grouping by namespace; a patch in none is a shell
    // of its own.
    let mut shells: Vec<(Option<&str>, Vec<Named<'_>>)> = Vec::new();
    for (handle, node) in scene
        .nodes()
        .filter(|(_, node)| node.node_type() == "nurbs")
    {
        let patch = match patch::read(node) {
            Ok(patch) => patch,
            Err(why) => {
                tessellation
                    .problems
                    .push(format!("ɴsɪ nurbs {handle:?} skipped: {why}"));
                continue;
            }
        };
        let namespace = namespace_of.get(handle).copied();
        match shells
            .iter_mut()
            .find(|(key, _)| namespace.is_some() && *key == namespace)
        {
            Some((_, patches)) => patches.push((handle, patch)),
            None => shells.push((namespace, vec![(handle, patch)])),
        }
    }

    for (_, patches) in shells {
        mesh_shell(patches, &welded, options, &mut tessellation);
    }
    tessellation
}

/// The weld use, as `(weld node, id)`, of each welded trim curve and
/// whole trim loop, by node.
#[derive(Default)]
struct Welded<'a> {
    curves: HashMap<(&'a str, usize), (String, u32)>,
    loops: HashMap<(&'a str, usize), (String, u32)>,
}

/// Splits each trim loop of `patch` into pieces: runs of consecutive
/// curves belonging to one weld use, and single unwelded curves.
fn pieces(handle: &str, patch: &Patch, welded: &Welded<'_>) -> Vec<Vec<Piece>> {
    let mut first_curve = 0;
    patch
        .loops
        .iter()
        .enumerate()
        .map(|(loop_index, curves)| {
            let whole_loop = welded.loops.get(&(handle, loop_index));
            let mut pieces: Vec<Piece> = Vec::new();
            for (offset, curve) in curves.iter().enumerate() {
                let weld = whole_loop
                    .or_else(|| {
                        welded.curves.get(&(handle, first_curve + offset))
                    })
                    .cloned();
                // A curve continues the previous piece when both belong to
                // the same weld use.
                match pieces.last_mut() {
                    Some(last) if weld.is_some() && last.weld == weld => {
                        if let Ok(joined) = last.curve.try_concat(curve) {
                            last.curve = joined;
                            continue;
                        }
                        pieces.push(Piece {
                            curve: curve.clone(),
                            weld,
                        });
                    }
                    _ => pieces.push(Piece {
                        curve: curve.clone(),
                        weld,
                    }),
                }
            }
            first_curve += curves.len();
            pieces
        })
        .collect()
}

/// Union-find over vertex slots.
struct Slots(Vec<usize>);

impl Slots {
    fn add(&mut self) -> usize {
        self.0.push(self.0.len());
        self.0.len() - 1
    }

    fn find(&mut self, slot: usize) -> usize {
        let parent = self.0[slot];
        if parent == slot {
            return slot;
        }
        let root = self.find(parent);
        self.0[slot] = root;
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.0[a] = b;
    }
}

fn point_at(curve: &EdgeCurve, t: f64) -> Point3 {
    curve.subs(t)
}

fn mesh_shell(
    patches: Vec<(&str, Patch)>,
    welded: &Welded<'_>,
    options: &NurbsOptions,
    tessellation: &mut NurbsTessellation,
) {
    let mut slots = Slots(Vec::new());
    // Per edge: its curve, and its start and end slots.
    let mut edges: Vec<(EdgeCurve, usize, usize)> = Vec::new();
    let mut edge_of_weld: HashMap<(String, u32), usize> = HashMap::default();
    let mut uses_of_edge: Vec<usize> = Vec::new();
    // Per face, per loop, per piece: (edge, orientation, face-local trim).
    let mut faces: Vec<Vec<Vec<(usize, bool, EdgeCurve)>>> = Vec::new();

    for (handle, patch) in &patches {
        let mut face_loops = Vec::new();
        for loop_pieces in pieces(handle, patch, welded) {
            // Each piece gets a start and end slot; consecutive pieces
            // share their junction, and the last closes onto the first.
            let piece_slots: Vec<(usize, usize)> = loop_pieces
                .iter()
                .map(|_| (slots.add(), slots.add()))
                .collect();
            for index in 0..piece_slots.len() {
                let next = (index + 1) % piece_slots.len();
                slots.union(piece_slots[index].1, piece_slots[next].0);
            }

            let mut uses = Vec::new();
            for (piece, &(start, end)) in
                loop_pieces.into_iter().zip(&piece_slots)
            {
                let local =
                    ParameterCurve::new(piece.curve, patch.surface.clone());
                let (t0, t1) = local.range_tuple();
                let existing = piece
                    .weld
                    .as_ref()
                    .and_then(|key| edge_of_weld.get(key))
                    .copied();
                let (edge, orientation) = match existing {
                    Some(edge) => {
                        // Direction only: which end of the shared edge this
                        // piece starts at. Identity came from the weld.
                        let (curve, edge_start, edge_end) = &edges[edge];
                        let (e0, e1) = curve.range_tuple();
                        // Ends and quarter points: the ends alone cannot
                        // tell the directions of a closed edge apart, nor
                        // can the middle -- both run through it.
                        let at = |curve: &EdgeCurve, (t0, t1): (f64, f64)| {
                            [0.0, 0.25, 0.75, 1.0]
                                .map(|f| point_at(curve, t0 + f * (t1 - t0)))
                        };
                        let this = at(&local, (t0, t1));
                        let edge_points = at(curve, (e0, e1));
                        let direct: f64 = (0..4)
                            .map(|i| distance2(this[i], edge_points[i]))
                            .sum();
                        let reversed: f64 = (0..4)
                            .map(|i| distance2(this[i], edge_points[3 - i]))
                            .sum();
                        let forward = direct <= reversed;
                        let (edge_start, edge_end) = (*edge_start, *edge_end);
                        if forward {
                            slots.union(start, edge_start);
                            slots.union(end, edge_end);
                        } else {
                            slots.union(start, edge_end);
                            slots.union(end, edge_start);
                        }
                        (edge, forward)
                    }
                    None => {
                        edges.push((local.clone(), start, end));
                        uses_of_edge.push(0);
                        let edge = edges.len() - 1;
                        if let Some(key) = piece.weld {
                            edge_of_weld.insert(key, edge);
                        }
                        (edge, true)
                    }
                };
                uses_of_edge[edge] += 1;
                uses.push((edge, orientation, local));
            }
            face_loops.push(uses);
        }
        faces.push(face_loops);
    }

    // Per face, the edges it uses; the welded ones' samples are the only
    // points its boundary may snap to.
    let edges_of_face: Vec<Vec<usize>> = faces
        .iter()
        .map(|loops| loops.iter().flatten().map(|use_| use_.0).collect())
        .collect();

    // One vertex per slot class, placed where its first edge puts it.
    let mut vertex_of_class: HashMap<usize, usize> = HashMap::default();
    let mut vertices: Vec<Point3> = Vec::new();
    let mut edge_vertices = Vec::with_capacity(edges.len());
    for (curve, start, end) in &edges {
        let (t0, t1) = curve.range_tuple();
        let mut vertex = |slot: usize, t: f64| {
            let class = slots.find(slot);
            *vertex_of_class.entry(class).or_insert_with(|| {
                vertices.push(point_at(curve, t));
                vertices.len() - 1
            })
        };
        edge_vertices.push((vertex(*start, t0), vertex(*end, t1)));
    }

    let shell = CompressedTrimmedShell {
        vertices,
        edges: edges
            .iter()
            .zip(&edge_vertices)
            .map(|((curve, _, _), &vertices)| CompressedEdge {
                vertices,
                curve: curve.clone(),
            })
            .collect(),
        faces: patches
            .iter()
            .zip(faces)
            .map(|((_, patch), loops)| CompressedTrimmedFace {
                boundaries: loops
                    .into_iter()
                    .map(|uses| {
                        uses.into_iter()
                            .map(|(index, orientation, trim)| {
                                CompressedEdgeUse {
                                    index,
                                    orientation,
                                    trim_curve: Some(trim),
                                }
                            })
                            .collect()
                    })
                    .collect(),
                // 3Delight's front side of a `nurbs` node is the side
                // ∂P/∂u × ∂P/∂v points to -- `monstertruck`'s own. Rendered:
                // see `tests/displacement.rs`.
                orientation: true,
                surface: patch.surface.clone(),
            })
            .collect(),
    };

    let meshed = trimmed_cshell_triangulation_with(
        &shell,
        TessellationOptions {
            tolerance: options.tolerance,
            ..Default::default()
        },
    );

    // Points on welded edges are the same samples on every face that uses
    // the edge; merge those, and only those.
    let welded_points: HashSet<[u64; 3]> = meshed
        .edges
        .iter()
        .zip(&uses_of_edge)
        .filter(|(_, uses)| **uses > 1)
        .flat_map(|(edge, _)| edge.curve.0.iter().map(|&point| bits(point)))
        .collect();

    let face_meshes: Vec<(&str, PolygonMesh)> = patches
        .iter()
        .zip(&meshed.faces)
        .zip(&edges_of_face)
        .filter_map(|(((handle, _), face), edges)| {
            let mut mesh = face.surface.clone()?;
            if !face.orientation {
                mesh.invert();
            }
            // A polyline's ends are resampled from its curve; the shell's
            // vertices are the one position its ends, and every other
            // edge's ends there, share.
            let samples: Vec<Point3> = edges
                .iter()
                .filter(|&&edge| uses_of_edge[edge] > 1)
                .flat_map(|&edge| {
                    let edge = &meshed.edges[edge];
                    let points = &edge.curve.0;
                    let inner = points
                        .get(1..points.len().saturating_sub(1))
                        .unwrap_or_default();
                    [
                        meshed.vertices[edge.vertices.0],
                        meshed.vertices[edge.vertices.1],
                    ]
                    .into_iter()
                    .chain(inner.iter().copied())
                })
                .collect();
            snap_boundary(&mut mesh, &samples, options.tolerance);
            Some((*handle, mesh))
        })
        .collect();
    for ((handle, _), face) in patches.iter().zip(&meshed.faces) {
        if face.surface.is_none() {
            tessellation.problems.push(format!(
                "ɴsɪ nurbs {handle:?}: the mesher produced no triangles"
            ));
        }
    }

    merge_and_emit(face_meshes, &welded_points, tessellation);
}

/// Moves vertices of `mesh` onto the nearest of `samples`, the shared
/// samples of the welded edges its face uses: a boundary vertex within
/// `tolerance`, any vertex within a hundredth of it.
///
/// The mesher places a face's boundary on those samples, but where it
/// re-evaluates them from their parameters, rounding moves them: close,
/// not bit-identical. Where a loop closes, it can also leave a second copy
/// of its first vertex a rounding away, inside a fold of slivers, so not
/// on the face's boundary. Only a face's own welded edges are candidates,
/// so identity still comes from the weld declarations; the distance only
/// undoes the rounding.
fn snap_boundary(mesh: &mut PolygonMesh, samples: &[Point3], tolerance: f64) {
    if samples.is_empty() {
        return;
    }
    let mut edge_uses: HashMap<(usize, usize), usize> = HashMap::default();
    for [a, b, c] in mesh.faces().triangle_iter() {
        for (from, to) in [(a.pos, b.pos), (b.pos, c.pos), (c.pos, a.pos)] {
            *edge_uses.entry((from.min(to), from.max(to))).or_default() += 1;
        }
    }
    let boundary: HashSet<usize> = edge_uses
        .into_iter()
        .filter(|&(_, uses)| uses == 1)
        .flat_map(|((a, b), _)| [a, b])
        .collect();
    let positions = mesh.positions_mut();
    for (vertex, position) in positions.iter_mut().enumerate() {
        let reach = if boundary.contains(&vertex) {
            tolerance
        } else {
            0.01 * tolerance
        };
        let point = *position;
        if let Some(&nearest) = samples.iter().min_by(|&&a, &&b| {
            distance2(point, a).total_cmp(&distance2(point, b))
        }) && distance2(point, nearest) <= reach * reach
        {
            *position = nearest;
        }
    }
}

fn distance2(a: Point3, b: Point3) -> f64 {
    let d = a - b;
    d.x * d.x + d.y * d.y + d.z * d.z
}

fn bits(point: Point3) -> [u64; 3] {
    [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()]
}

/// Merges welded points across faces, computes normals on the merged
/// surface, counts its open edges, and splits it back into one mesh per
/// node.
fn merge_and_emit(
    face_meshes: Vec<(&str, PolygonMesh)>,
    welded_points: &HashSet<[u64; 3]>,
    tessellation: &mut NurbsTessellation,
) {
    // Global vertex ids: a welded point is one id wherever it occurs.
    let mut global_of_point: HashMap<[u64; 3], usize> = HashMap::default();
    let mut global_positions: Vec<Point3> = Vec::new();
    // Per face: its local-to-global map and triangles in global ids.
    let mut faces: Vec<FaceTriangles<'_>> = Vec::new();

    for (handle, mesh) in &face_meshes {
        let local_to_global: Vec<usize> = mesh
            .positions()
            .iter()
            .map(|&point| {
                let key = bits(point);
                if welded_points.contains(&key) {
                    *global_of_point.entry(key).or_insert_with(|| {
                        global_positions.push(point);
                        global_positions.len() - 1
                    })
                } else {
                    global_positions.push(point);
                    global_positions.len() - 1
                }
            })
            .collect();
        // A triangle whose corners merged into one another is gone.
        let triangles = mesh
            .faces()
            .triangle_iter()
            .map(|[a, b, c]| {
                [
                    local_to_global[a.pos],
                    local_to_global[b.pos],
                    local_to_global[c.pos],
                ]
            })
            .filter(|[a, b, c]| a != b && b != c && c != a)
            .collect();
        faces.push((handle, local_to_global, triangles));
    }

    // Area-weighted normals over the merged surface.
    let mut normals = vec![[0.0f64; 3]; global_positions.len()];
    let mut edge_uses: HashMap<(usize, usize), usize> = HashMap::default();
    for (_, _, triangles) in &faces {
        for &[a, b, c] in triangles {
            let (pa, pb, pc) = (
                global_positions[a],
                global_positions[b],
                global_positions[c],
            );
            let normal = (pb - pa).cross(pc - pa);
            for vertex in [a, b, c] {
                normals[vertex][0] += normal.x;
                normals[vertex][1] += normal.y;
                normals[vertex][2] += normal.z;
            }
            for (from, to) in [(a, b), (b, c), (c, a)] {
                *edge_uses.entry((from.min(to), from.max(to))).or_default() +=
                    1;
            }
        }
    }
    tessellation.open_edges +=
        edge_uses.values().filter(|&&uses| uses == 1).count();
    for normal in &mut normals {
        let length = (normal[0] * normal[0]
            + normal[1] * normal[1]
            + normal[2] * normal[2])
            .sqrt();
        if length > 0.0 {
            normal.iter_mut().for_each(|component| *component /= length);
        }
    }

    for (handle, local_to_global, triangles) in faces {
        // Re-index per node: its own vertices, in first-use order.
        let mut local_of_global: HashMap<usize, u32> = HashMap::default();
        let mut mesh = NurbsMesh {
            geometry: handle.to_string(),
            ..NurbsMesh::default()
        };
        for global in local_to_global {
            local_of_global.entry(global).or_insert_with(|| {
                let point = global_positions[global];
                mesh.positions.push([point.x, point.y, point.z]);
                mesh.normals.push(normals[global]);
                (mesh.positions.len() - 1) as u32
            });
        }
        mesh.triangles = triangles
            .into_iter()
            .map(|triangle| triangle.map(|global| local_of_global[&global]))
            .collect();
        tessellation.meshes.push(mesh);
    }
}
