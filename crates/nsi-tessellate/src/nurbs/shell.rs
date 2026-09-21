//! Patches of one weld namespace, meshed as one shell.

use super::patch::{self, Patch, Surface, TrimCurve};
use ahash::{AHashMap as HashMap, AHashSet as HashSet};
use monstertruck::{
    geometry::prelude::{
        BsplineCurve, KnotVector, NurbsCurve, ParameterCurve, Point3, Vector3,
    },
    meshing::prelude::{
        PolygonMesh, TessellationOptions, trimmed_cshell_triangulation_with,
    },
    topology::compress::{
        CompressedEdge, CompressedEdgeUse, CompressedTrimmedFace,
        CompressedTrimmedShell,
    },
    traits::{BoundedCurve, Concat, Cut, Invertible, ParametricCurve},
};
use nsi_intermediate::{NurbsSide, Scene, WeldKind};

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
    /// The use it belongs to, if it is welded, and whether this piece --
    /// as it is stored, in loop order -- runs against that weld's
    /// reference traversal.
    weld: Option<Welding>,
    /// Which part of its weld's boundary it is: 0, unless the uses of a
    /// closed weld start at different points and it was split there.
    part: usize,
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

    // Which weld use covers which trim curve, whole trim loop, or side of
    // the active domain -- or which part of one -- of which node.
    let mut welded = Welded::default();
    let mut namespace_of: HashMap<&str, &str> = HashMap::default();
    for weld in &welds.welds {
        for weld_use in &weld.uses {
            namespace_of.insert(weld_use.geometry, weld.weld);
            let key = (weld.weld.to_string(), weld.id);
            let has_side = weld_use.segments.iter().any(|segment| {
                matches!(segment.kind, WeldKind::NurbsSide { .. })
            });
            let has_trim = weld_use.segments.iter().any(|segment| {
                matches!(
                    segment.kind,
                    WeldKind::TrimCurve { .. } | WeldKind::TrimLoop { .. }
                )
            });
            // The contract allows a chain through both, where the two
            // are consecutive portions of one retained boundary. This
            // mesher builds boundaries loop by loop, and a patch's sides
            // and its trim loops are different loops, so it reports the
            // limitation rather than weld half of such a use.
            if has_side && has_trim {
                tessellation.problems.push(format!(
                    "ɴsɪ weld {:?} {} on {:?}: a use through both a side \
                     and trim curves is not welded by this mesher; that \
                     boundary stays unwelded",
                    weld.weld, weld.id, weld_use.geometry
                ));
                continue;
            }
            for segment in &weld_use.segments {
                let range = segment.range.map(f64::from);
                match segment.kind {
                    WeldKind::TrimCurve { curve_index } => welded
                        .curves
                        .entry((weld_use.geometry, curve_index))
                        .or_default()
                        .push(Selection {
                            range,
                            weld: key.clone(),
                            reverse: segment.reverse,
                        }),
                    WeldKind::TrimLoop { loop_index } => {
                        welded.loops.insert(
                            (weld_use.geometry, loop_index),
                            (key.clone(), segment.reverse),
                        );
                    }
                    WeldKind::NurbsSide { side } => welded
                        .sides
                        .entry(weld_use.geometry)
                        .or_default()[side_index(side)]
                    .push(Selection {
                        range,
                        weld: key.clone(),
                        reverse: segment.reverse,
                    }),
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

/// A weld use's hold on part of a curve or side: the part, normalized to
/// `[0, 1]` along the selector's own direction, the use as
/// `(weld node, id)`, and whether the use traverses it backwards.
#[derive(Clone)]
struct Selection {
    range: [f64; 2],
    weld: (String, u32),
    /// `weld.reverse`: the selected segment is traversed against its own
    /// direction to follow the weld's reference traversal.
    reverse: bool,
}

/// The weld uses of each welded trim curve, whole trim loop, and side of
/// the active domain, by node. Sides are indexed as the draft numbers them.
#[derive(Default)]
struct Welded<'a> {
    curves: HashMap<(&'a str, usize), Vec<Selection>>,
    loops: HashMap<(&'a str, usize), Welding>,
    sides: HashMap<&'a str, [Vec<Selection>; 4]>,
}

/// A piece's weld: the use it belongs to, and whether the piece runs
/// against that weld's reference traversal.
type Welding = ((String, u32), bool);

/// The draft's number for `side`.
fn side_index(side: NurbsSide) -> usize {
    match side {
        NurbsSide::UMin => 0,
        NurbsSide::UMax => 1,
        NurbsSide::VMin => 2,
        NurbsSide::VMax => 3,
    }
}

/// Splits each trim loop of `patch` into pieces: runs of consecutive
/// curves, or parts of curves, belonging to one weld use, and unwelded
/// curves. A patch with welded sides gets its active domain's outline as
/// one more loop, the first.
fn pieces(handle: &str, patch: &Patch, welded: &Welded<'_>) -> Vec<Vec<Piece>> {
    let mut first_curve = 0;
    let trim_loops =
        patch.loops.iter().enumerate().map(|(loop_index, curves)| {
            let whole_loop = welded.loops.get(&(handle, loop_index));
            let pieces = curves.iter().enumerate().fold(
                Vec::new(),
                |mut pieces, (offset, curve)| {
                    let parts = match whole_loop {
                        Some(weld) => vec![(curve.clone(), Some(weld.clone()))],
                        None => split(
                            curve,
                            welded
                                .curves
                                .get(&(handle, first_curve + offset))
                                .map_or(&[][..], Vec::as_slice),
                        ),
                    };
                    parts.into_iter().for_each(|(curve, weld)| {
                        push_piece(&mut pieces, curve, weld)
                    });
                    pieces
                },
            );
            first_curve += curves.len();
            pieces
        });
    welded
        .sides
        .get(handle)
        .map(|sides| domain_loop(patch.domain, sides))
        .into_iter()
        .chain(trim_loops)
        .collect()
}

/// Appends `curve` to `pieces`, joining it to the last piece when both
/// belong to the same weld use and run the same way along it.
fn push_piece(
    pieces: &mut Vec<Piece>,
    curve: TrimCurve,
    weld: Option<Welding>,
) {
    let joined = pieces
        .last()
        .filter(|last| weld.is_some() && last.weld == weld)
        .and_then(|last| last.curve.try_concat(&curve).ok());
    match (joined, pieces.last_mut()) {
        (Some(joined), Some(last)) => last.curve = joined,
        _ => pieces.push(Piece {
            curve,
            weld,
            part: 0,
        }),
    }
}

/// Splits `curve` where the selections' ranges begin and end, and gives
/// each part the use whose range covers it.
fn split(
    curve: &TrimCurve,
    selections: &[Selection],
) -> Vec<(TrimCurve, Option<Welding>)> {
    let (t0, t1) = curve.range_tuple();
    let mut breaks: Vec<f64> = selections
        .iter()
        .flat_map(|selection| selection.range)
        .filter(|&at| 0.0 < at && at < 1.0)
        .collect();
    breaks.sort_by(f64::total_cmp);
    breaks.dedup();
    let owner = |from: f64, to: f64| {
        let middle = 0.5 * (from + to);
        selections
            .iter()
            .find(|selection| {
                let [a, b] = selection.range;
                a.min(b) <= middle && middle <= a.max(b)
            })
            .map(|selection| (selection.weld.clone(), selection.reverse))
    };
    let mut rest = curve.clone();
    let mut from = 0.0;
    let mut parts: Vec<_> = breaks
        .into_iter()
        .map(|at| {
            let tail = rest.cut(t0 + at * (t1 - t0));
            let part = (std::mem::replace(&mut rest, tail), owner(from, at));
            from = at;
            part
        })
        .collect();
    parts.push((rest, owner(from, 1.0)));
    parts
}

/// The outline of the active `domain`, counter-clockwise in `(u, v)`:
/// the v-min, u-max, v-max and u-min sides, each split by the uses that
/// select parts of it. A u-side runs along increasing `v` and a v-side
/// along increasing `u`, so the loop runs the last two backwards.
fn domain_loop(
    ((u0, u1), (v0, v1)): ((f64, f64), (f64, f64)),
    sides: &[Vec<Selection>; 4],
) -> Vec<Piece> {
    [
        (2, (u0, v0), (u1, v0), false),
        (1, (u1, v0), (u1, v1), false),
        (3, (u0, v1), (u1, v1), true),
        (0, (u0, v0), (u0, v1), true),
    ]
    .into_iter()
    .fold(Vec::new(), |mut pieces, (side, from, to, backwards)| {
        let mut parts = split(&line(from, to), &sides[side]);
        if backwards {
            parts.reverse();
            parts.iter_mut().for_each(|(curve, weld)| {
                curve.invert();
                // Stored the other way round than the side runs, so a use
                // that followed the side now runs against its weld.
                if let Some((_, against)) = weld {
                    *against = !*against;
                }
            });
        }
        parts
            .into_iter()
            .for_each(|(curve, weld)| push_piece(&mut pieces, curve, weld));
        pieces
    })
}

/// A straight trim curve from `from` to `to`.
fn line(from: (f64, f64), to: (f64, f64)) -> TrimCurve {
    NurbsCurve::new(BsplineCurve::new(
        KnotVector::bezier_knot(1),
        vec![
            Vector3::new(from.0, from.1, 1.0),
            Vector3::new(to.0, to.1, 1.0),
        ],
    ))
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
            slot
        } else {
            let root = self.find(parent);
            self.0[slot] = root;
            root
        }
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
    // Per edge: its curve, its start and end slots, and whether the use
    // that made it runs against its weld's reference traversal.
    let mut edges: Vec<(EdgeCurve, usize, usize, bool)> = Vec::new();
    let mut edge_of_weld: HashMap<((String, u32), usize), usize> =
        HashMap::default();
    let mut uses_of_edge: Vec<usize> = Vec::new();
    // Per face, per loop, per piece: (edge, orientation, face-local trim).
    let mut faces: Vec<Vec<Vec<(usize, bool, EdgeCurve)>>> = Vec::new();

    let mut face_pieces: Vec<Vec<Vec<Piece>>> = patches
        .iter()
        .map(|(handle, patch)| pieces(handle, patch, welded))
        .collect();
    split_at_anchors(&patches, &mut face_pieces, options.tolerance);

    for ((_, patch), loops) in patches.iter().zip(face_pieces) {
        let mut face_loops = Vec::new();
        for loop_pieces in loops {
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
                let existing = piece
                    .weld
                    .as_ref()
                    .and_then(|(key, _)| {
                        edge_of_weld.get(&(key.clone(), piece.part))
                    })
                    .copied();
                let (edge, orientation) = match existing {
                    Some(edge) => {
                        // Declared, not measured. Every use of a weld
                        // follows one reference traversal once its
                        // ranges, segment order and `weld.reverse` are
                        // applied, so two pieces run the same way along
                        // their edge exactly when they sit the same way
                        // against that traversal. A closed edge's ends
                        // are one point and cannot tell the two senses
                        // apart, which is why the contract forbids
                        // inferring direction from them.
                        let against = piece
                            .weld
                            .as_ref()
                            .is_some_and(|(_, against)| *against);
                        let forward = against == edges[edge].3;
                        let (_, edge_start, edge_end, _) = edges[edge];
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
                        let against = piece
                            .weld
                            .as_ref()
                            .is_some_and(|(_, against)| *against);
                        edges.push((local.clone(), start, end, against));
                        uses_of_edge.push(0);
                        let edge = edges.len() - 1;
                        if let Some((key, _)) = piece.weld {
                            edge_of_weld.insert((key, piece.part), edge);
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
    for (curve, start, end, _) in &edges {
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
            .map(|((curve, _, _, _), &vertices)| CompressedEdge {
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
    // A face with no welded edges has nothing to snap to.
    if !samples.is_empty() {
        let mut edge_uses: HashMap<(usize, usize), usize> = HashMap::default();
        for [a, b, c] in mesh.faces().triangle_iter() {
            for (from, to) in [(a.pos, b.pos), (b.pos, c.pos), (c.pos, a.pos)] {
                *edge_uses.entry((from.min(to), from.max(to))).or_default() +=
                    1;
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
}

/// Splits the uses of every closed weld whose uses start at different
/// points, each at every other use's start.
///
/// The contract lets a closed weld's uses start anywhere -- a periodic
/// patch's side starts at its seam, wherever the source edge began -- and
/// leaves matching them to the renderer. Left whole, the shared edge has
/// one face's start as its only vertex, and the other face's start lands
/// part way along it: one point the two do not share, and a crack. Split
/// at every start, each use is a run of open arcs whose ends both faces
/// hold. Which arc is which is found by position, which is the renderer's
/// to establish once the declaration has said the two are one boundary.
fn split_at_anchors(
    patches: &[(&str, Patch)],
    faces: &mut [Vec<Vec<Piece>>],
    tolerance: f64,
) {
    let limit = tolerance * tolerance;
    let local = |face: usize, piece: &Piece| {
        ParameterCurve::new(
            piece.curve.clone(),
            patches[face].1.surface.clone(),
        )
    };

    // Every welded use: its weld, the face it lies on, where it starts,
    // and whether it comes back there.
    let uses: Vec<((String, u32), usize, Point3, bool)> = faces
        .iter()
        .enumerate()
        .flat_map(|(face, loops)| {
            loops.iter().flatten().filter_map(move |piece| {
                piece.weld.as_ref().map(|(key, _)| (face, piece, key))
            })
        })
        .map(|(face, piece, key)| {
            let curve = local(face, piece);
            let (t0, t1) = curve.range_tuple();
            let (start, end) = (point_at(&curve, t0), point_at(&curve, t1));
            (key.clone(), face, start, distance2(start, end) <= limit)
        })
        .collect();

    // The welds whose uses are all closed and do not all start at one
    // point: their distinct starts, and the midpoints of the arcs between
    // them along the first use.
    let plans: HashMap<(String, u32), (Vec<Point3>, Vec<Point3>)> = uses
        .iter()
        .map(|(key, ..)| key)
        .collect::<HashSet<_>>()
        .into_iter()
        .filter_map(|key| {
            let of_weld: Vec<_> =
                uses.iter().filter(|(other, ..)| other == key).collect();
            let anchors = of_weld.iter().fold(
                Vec::new(),
                |mut anchors: Vec<Point3>, (_, _, start, _)| {
                    if anchors
                        .iter()
                        .all(|anchor| distance2(*anchor, *start) > limit)
                    {
                        anchors.push(*start);
                    }
                    anchors
                },
            );
            let (_, face, ..) = of_weld[0];
            let splits =
                anchors.len() > 1 && of_weld.iter().all(|(.., closed)| *closed);
            faces[*face]
                .iter()
                .flatten()
                .find(|piece| {
                    piece.weld.as_ref().is_some_and(|(other, _)| other == key)
                })
                .filter(|_| splits)
                .map(|first| {
                    let curve = local(*face, first);
                    let midpoints = arcs(&curve, &anchors, limit)
                        .iter()
                        .map(|&(from, to)| point_at(&curve, 0.5 * (from + to)))
                        .collect();
                    (key.clone(), (anchors, midpoints))
                })
        })
        .collect();

    faces.iter_mut().enumerate().for_each(|(face, loops)| {
        loops.iter_mut().for_each(|loop_pieces| {
            *loop_pieces = loop_pieces
                .drain(..)
                .flat_map(|piece| {
                    match piece
                        .weld
                        .as_ref()
                        .and_then(|(key, _)| plans.get(key))
                    {
                        Some((anchors, midpoints)) => split_piece(
                            &local(face, &piece),
                            piece,
                            anchors,
                            midpoints,
                            limit,
                        ),
                        None => vec![piece],
                    }
                })
                .collect();
        });
    });
}

/// `piece` as the arcs between `anchors`, each tagged with the part of
/// its weld whose midpoint -- among `midpoints` -- lies nearest its own.
fn split_piece(
    curve: &EdgeCurve,
    piece: Piece,
    anchors: &[Point3],
    midpoints: &[Point3],
    limit: f64,
) -> Vec<Piece> {
    let mut rest = piece.curve;
    arcs(curve, anchors, limit)
        .into_iter()
        .map(|(from, to)| {
            // What lies before `to` is this arc; the rest goes on.
            let arc = if to < rest.range_tuple().1 {
                let tail = rest.cut(to);
                std::mem::replace(&mut rest, tail)
            } else {
                rest.clone()
            };
            let middle = point_at(curve, 0.5 * (from + to));
            let part = midpoints
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    distance2(**a, middle).total_cmp(&distance2(**b, middle))
                })
                .map_or(0, |(part, _)| part);
            Piece {
                curve: arc,
                weld: piece.weld.clone(),
                part,
            }
        })
        .collect()
}

/// The arcs of the closed `curve` between the given points, as parameter
/// intervals in its own order.
fn arcs(curve: &EdgeCurve, points: &[Point3], limit: f64) -> Vec<(f64, f64)> {
    let (t0, t1) = curve.range_tuple();
    let start = point_at(curve, t0);
    let mut cuts: Vec<f64> = points
        .iter()
        .filter(|point| distance2(**point, start) > limit)
        .map(|point| nearest_parameter(curve, *point))
        .filter(|&t| t0 < t && t < t1)
        .collect();
    cuts.sort_by(f64::total_cmp);
    let bounds: Vec<f64> = std::iter::once(t0)
        .chain(cuts)
        .chain(std::iter::once(t1))
        .collect();
    bounds.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// The parameter of `curve` nearest `point`: the nearest of evenly spaced
/// samples, refined by ternary search around it.
fn nearest_parameter(curve: &EdgeCurve, point: Point3) -> f64 {
    const SAMPLES: usize = 64;
    let (t0, t1) = curve.range_tuple();
    let step = (t1 - t0) / SAMPLES as f64;
    let distance = |t: f64| distance2(point_at(curve, t), point);
    let nearest = (0..=SAMPLES)
        .map(|at| t0 + at as f64 * step)
        .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
        .unwrap_or(t0);
    let (mut low, mut high) =
        ((nearest - step).max(t0), (nearest + step).min(t1));
    for _ in 0..64 {
        let third = (high - low) / 3.0;
        if distance(low + third) < distance(high - third) {
            high -= third;
        } else {
            low += third;
        }
    }
    0.5 * (low + high)
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
