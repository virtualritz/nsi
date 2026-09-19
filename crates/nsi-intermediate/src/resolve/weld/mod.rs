//! Weld declarations: which boundaries of which surfaces are one edge.
//!
//! The ɴsɪ draft [Shared Boundaries: Weld Declarations][draft] has a
//! `weld` node act as a namespace. Each geometry connected to one carries
//! a table of *boundary uses*; two uses with the same `(weld node, id)`
//! are one joined boundary. This module reads and validates those tables
//! and groups the uses. How a join survives tessellation is the
//! tessellator's business -- see `nsi-tessellate`.
//!
//! [draft]: https://nsi.readthedocs.io/en/latest/design/shared-boundaries.html

use super::*;
use core::fmt;

/// The attribute a geometry's `weld` connection arrives on.
const WELD: &str = "weld";

/// Which boundary a segment selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WeldKind {
    /// A complete trim loop of a `nurbs` node, in its stored curve order.
    TrimLoop {
        /// The loop, counted over `trimcurves.ncurves`.
        loop_index: usize,
    },
    /// One curve in the flattened trim-curve arrays of a `nurbs` node.
    TrimCurve {
        /// The curve, counted over all loops.
        curve_index: usize,
    },
    /// One side of a `nurbs` node's active domain.
    NurbsSide {
        /// Which side.
        side: NurbsSide,
    },
    /// One directed edge of a polygon or subdivision `mesh` face.
    MeshEdge {
        /// The face.
        face_index: usize,
        /// The face's loop: 0 is the outer perimeter, later loops holes.
        loop_index: usize,
        /// The edge, from a loop vertex to the next one.
        edge_index: usize,
    },
}

/// A side of a `nurbs` node's active parametric domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NurbsSide {
    /// Where `u` is smallest; runs along increasing `v`.
    UMin,
    /// Where `u` is largest; runs along increasing `v`.
    UMax,
    /// Where `v` is smallest; runs along increasing `u`.
    VMin,
    /// Where `v` is largest; runs along increasing `u`.
    VMax,
}

/// One segment of a boundary use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeldSegment {
    /// What the segment selects.
    pub kind: WeldKind,
    /// Whether the segment is traversed against its own direction.
    pub reverse: bool,
    /// The selected portion, normalized to `[0, 1]` over the selected
    /// curve or edge, before reversal. `[0, 1]` is all of it.
    pub range: [f32; 2],
}

/// One boundary use: a connected, ordered chain on one geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct WeldUse<'a> {
    /// The geometry the chain lies on.
    pub geometry: &'a str,
    /// The `weld` node whose namespace `id` belongs to.
    pub weld: &'a str,
    /// The identity within that namespace.
    pub id: u32,
    /// The use's position in its geometry's table.
    pub use_index: usize,
    /// The chain, in order.
    pub segments: Vec<WeldSegment>,
}

/// One joined boundary: every use that shares a `(weld node, id)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Weld<'a> {
    /// The namespace.
    pub weld: &'a str,
    /// The identity within it.
    pub id: u32,
    /// The uses, in scene order.
    pub uses: Vec<WeldUse<'a>>,
}

impl Weld<'_> {
    /// A use without a counterpart: it joins nothing by itself.
    pub fn is_open(&self) -> bool {
        self.uses.len() < 2
    }

    /// More than two uses: a non-manifold join.
    pub fn is_non_manifold(&self) -> bool {
        2 < self.uses.len()
    }
}

/// A geometry's weld table, read and checked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeldTable<'a> {
    /// The valid uses.
    pub uses: Vec<WeldUse<'a>>,
    /// Everything wrong with the table. A problem that invalidates a
    /// use drops that use; see [`WeldProblem::drops_use`].
    pub problems: Vec<WeldProblem<'a>>,
}

/// Every joined boundary in a scene.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Welds<'a> {
    /// The joined boundaries, grouped by `(weld node, id)`.
    pub welds: Vec<Weld<'a>>,
    /// Every problem found in any geometry's table.
    pub problems: Vec<WeldProblem<'a>>,
}

/// Something wrong with a weld table.
#[derive(Debug, Clone, PartialEq)]
pub struct WeldProblem<'a> {
    /// The geometry whose table it is.
    pub geometry: &'a str,
    /// The use, when the problem is in one.
    pub use_index: Option<usize>,
    /// What is wrong.
    pub kind: WeldProblemKind,
}

/// What is wrong with a weld table.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum WeldProblemKind {
    /// The draft allows one `weld` connection per geometry.
    SeveralWeldNodes {
        /// How many there are.
        count: usize,
    },
    /// Weld attributes, but no `weld` connection to give their ids a
    /// namespace.
    NoWeldNode,
    /// A table attribute is missing, of the wrong type, or has the wrong
    /// number of values.
    BadAttribute {
        /// The attribute.
        attribute: &'static str,
        /// The number of values it should have, if it has another.
        expected: Option<usize>,
    },
    /// A negative id.
    NegativeId {
        /// The id.
        id: i32,
    },
    /// A use with no segments.
    EmptyUse,
    /// A `weld.kind` the draft does not define, or one the geometry's
    /// type cannot have.
    UnknownKind {
        /// The kind as written.
        kind: String,
    },
    /// A `weld.index` that selects nothing on this geometry.
    BadIndex {
        /// The index as written.
        index: [i32; 3],
    },
    /// A `weld.range` outside `[0, 1]`, or empty.
    BadRange {
        /// The range as written.
        range: [f32; 2],
    },
    /// A `trim-loop` segment that is not a complete use on its own: it
    /// has company, or a partial range.
    PartialLoop,
    /// Consecutive segments that do not meet.
    NotConnected {
        /// The segment that does not continue the one before it.
        segment: usize,
    },
    /// A chain whose connectivity cannot be checked from topology --
    /// segments of different kinds, of different trim loops, or partial
    /// ranges. The use is kept.
    ConnectivityUnverified,
}

impl WeldProblem<'_> {
    /// Whether the problem makes the use unusable, so it is not in
    /// [`WeldTable::uses`].
    pub fn drops_use(&self) -> bool {
        !matches!(self.kind, WeldProblemKind::ConnectivityUnverified)
    }
}

impl fmt::Display for WeldProblem<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ɴsɪ weld table on {:?}", self.geometry)?;
        if let Some(index) = self.use_index {
            write!(f, ", use {index}")?;
        }
        f.write_str(": ")?;
        match &self.kind {
            WeldProblemKind::SeveralWeldNodes { count } => write!(
                f,
                "{count} weld connections; a geometry takes one namespace"
            ),
            WeldProblemKind::NoWeldNode => f.write_str(
                "weld attributes without a weld connection; the ids have no \
                 namespace",
            ),
            WeldProblemKind::BadAttribute {
                attribute,
                expected: Some(expected),
            } => write!(f, "{attribute:?} should have {expected} values"),
            WeldProblemKind::BadAttribute {
                attribute,
                expected: None,
            } => write!(f, "{attribute:?} is missing or of the wrong type"),
            WeldProblemKind::NegativeId { id } => {
                write!(f, "id {id} is negative")
            }
            WeldProblemKind::EmptyUse => f.write_str("the use has no segments"),
            WeldProblemKind::UnknownKind { kind } => {
                write!(f, "{kind:?} is not a boundary this geometry has")
            }
            WeldProblemKind::BadIndex { index } => {
                write!(f, "index {index:?} selects nothing on this geometry")
            }
            WeldProblemKind::BadRange { range } => {
                write!(f, "range {range:?} is not a sub-interval of [0, 1]")
            }
            WeldProblemKind::PartialLoop => f.write_str(
                "a trim-loop segment must be a whole use with its full range",
            ),
            WeldProblemKind::NotConnected { segment } => write!(
                f,
                "segment {segment} does not continue the chain before it"
            ),
            WeldProblemKind::ConnectivityUnverified => f.write_str(
                "the chain's connectivity cannot be checked from topology; \
                 kept as declared",
            ),
        }
    }
}

/// What a geometry has that a segment can select.
enum Boundaries {
    Nurbs {
        /// Curves per trim loop, `trimcurves.ncurves`.
        curves_per_loop: Vec<usize>,
    },
    Mesh {
        /// Per face, the vertex count of each of its loops.
        loops_per_face: Vec<Vec<usize>>,
    },
    None,
}

impl Boundaries {
    fn of(node: &Node) -> Self {
        let counts = |name: &str| {
            node.attribute(name).and_then(OwnedArgument::as_i32s).map(
                |values| {
                    values
                        .iter()
                        .map(|&v| v.max(0) as usize)
                        .collect::<Vec<_>>()
                },
            )
        };
        match node.node_type() {
            "nurbs" => Self::Nurbs {
                curves_per_loop: counts("trimcurves.ncurves")
                    .unwrap_or_default(),
            },
            "mesh" => {
                let vertices = counts("nvertices").unwrap_or_default();
                let loops_per_face = match counts("nholes") {
                    // With holes, each face takes its outer perimeter
                    // plus one count per hole.
                    Some(holes) => {
                        let mut counts = vertices.into_iter();
                        holes
                            .into_iter()
                            .map(|holes| {
                                counts.by_ref().take(holes + 1).collect()
                            })
                            .collect()
                    }
                    None => {
                        vertices.into_iter().map(|count| vec![count]).collect()
                    }
                };
                Self::Mesh { loops_per_face }
            }
            _ => Self::None,
        }
    }

    /// The segment kind `kind` with `index`, if this geometry has it.
    fn select(
        &self,
        kind: &[u8],
        index: [i32; 3],
    ) -> Result<WeldKind, WeldProblemKind> {
        let unknown = || WeldProblemKind::UnknownKind {
            kind: String::from_utf8_lossy(kind).into_owned(),
        };
        let bad = || WeldProblemKind::BadIndex { index };
        let at = |position: usize| {
            usize::try_from(index[position]).map_err(|_| bad())
        };
        let unused_zero = |from: usize| {
            if index[from..].iter().all(|&v| v == 0) {
                Ok(())
            } else {
                Err(bad())
            }
        };

        match (self, kind) {
            (Self::Nurbs { curves_per_loop }, b"trim-loop") => {
                let loop_index = at(0)?;
                unused_zero(1)?;
                (loop_index < curves_per_loop.len())
                    .then_some(WeldKind::TrimLoop { loop_index })
                    .ok_or_else(bad)
            }
            (Self::Nurbs { curves_per_loop }, b"trim-curve") => {
                let curve_index = at(0)?;
                unused_zero(1)?;
                (curve_index < curves_per_loop.iter().sum())
                    .then_some(WeldKind::TrimCurve { curve_index })
                    .ok_or_else(bad)
            }
            (Self::Nurbs { .. }, b"nurbs-side") => {
                unused_zero(1)?;
                let side = match at(0)? {
                    0 => NurbsSide::UMin,
                    1 => NurbsSide::UMax,
                    2 => NurbsSide::VMin,
                    3 => NurbsSide::VMax,
                    _ => return Err(bad()),
                };
                Ok(WeldKind::NurbsSide { side })
            }
            (Self::Mesh { loops_per_face }, b"mesh-edge") => {
                let (face_index, loop_index, edge_index) =
                    (at(0)?, at(1)?, at(2)?);
                let vertices = loops_per_face
                    .get(face_index)
                    .and_then(|loops| loops.get(loop_index))
                    .ok_or_else(bad)?;
                (edge_index < *vertices)
                    .then_some(WeldKind::MeshEdge {
                        face_index,
                        loop_index,
                        edge_index,
                    })
                    .ok_or_else(bad)
            }
            _ => Err(unknown()),
        }
    }

    /// The loop a trim curve belongs to, and the loop's first curve and
    /// length.
    fn loop_of(&self, curve_index: usize) -> Option<(usize, usize, usize)> {
        let Self::Nurbs { curves_per_loop } = self else {
            return None;
        };
        let mut first = 0;
        for (loop_index, &count) in curves_per_loop.iter().enumerate() {
            if curve_index < first + count {
                return Some((loop_index, first, count));
            }
            first += count;
        }
        None
    }
}

/// Whether `next` continues `previous` in a chain of whole trim curves
/// of one loop: the next curve along the loop, or the previous one when
/// both are reversed, with wraparound.
fn trim_curves_continue(
    boundaries: &Boundaries,
    previous: &WeldSegment,
    next: &WeldSegment,
) -> Option<bool> {
    let (
        WeldKind::TrimCurve { curve_index: a },
        WeldKind::TrimCurve { curve_index: b },
    ) = (previous.kind, next.kind)
    else {
        return None;
    };
    let whole = |segment: &WeldSegment| segment.range == [0.0, 1.0];
    let (loop_a, first, count) = boundaries.loop_of(a)?;
    let (loop_b, ..) = boundaries.loop_of(b)?;
    if loop_a != loop_b
        || !whole(previous)
        || !whole(next)
        || previous.reverse != next.reverse
    {
        return None;
    }
    let step = |from: usize, forward: bool| {
        let local = from - first;
        first
            + if forward {
                (local + 1) % count
            } else {
                (local + count - 1) % count
            }
    };
    Some(b == step(a, !previous.reverse))
}

impl Scene {
    /// The weld table of `geometry`, read and checked against the
    /// geometry's own boundaries.
    ///
    /// Empty, with no problems, for a geometry that declares no welds.
    pub fn weld_table(&self, geometry: &str) -> WeldTable<'_> {
        let Some((handle, node)) = self.node_entry(geometry) else {
            return WeldTable::default();
        };
        let mut uses = Vec::new();
        let mut problems = Vec::new();
        // A macro rather than a closure: a closure would hold `problems`
        // borrowed across the whole function.
        macro_rules! problem {
            ($use_index: expr, $kind: expr $(,)?) => {
                problems.push(WeldProblem {
                    geometry: handle,
                    use_index: $use_index,
                    kind: $kind,
                })
            };
        }
        // Every early exit hands back what was found so far.
        macro_rules! done {
            () => {
                return WeldTable { uses, problems }
            };
        }

        let ids = node.attribute("weld.id");
        let welds = self
            .edges_to_attribute(handle, WELD)
            .filter(|edge| edge.kind == EdgeKind::Weld)
            .collect::<Vec<_>>();

        // No table: nothing declared. A `weld` connection without one is a
        // namespace with nothing in it, which is harmless.
        let Some(ids) = ids else { done!() };
        let weld = match welds.as_slice() {
            [weld] => weld.from(),
            [] => {
                problem!(None, WeldProblemKind::NoWeldNode);
                done!();
            }
            several => {
                problem!(
                    None,
                    WeldProblemKind::SeveralWeldNodes {
                        count: several.len(),
                    },
                );
                done!();
            }
        };

        let Some(ids) = ids.as_i32s() else {
            problem!(
                None,
                WeldProblemKind::BadAttribute {
                    attribute: "weld.id",
                    expected: None,
                },
            );
            done!();
        };
        let use_count = ids.len();

        let counts = match node.attribute("weld.segment-count") {
            None => vec![1; use_count],
            Some(counts) => match counts.as_i32s() {
                Some(values) if values.len() == use_count => {
                    values.iter().map(|&v| v.max(0) as usize).collect()
                }
                _ => {
                    problem!(
                        None,
                        WeldProblemKind::BadAttribute {
                            attribute: "weld.segment-count",
                            expected: Some(use_count),
                        },
                    );
                    done!();
                }
            },
        };
        let segments: usize = counts.iter().sum();

        let kinds = node
            .attribute("weld.kind")
            .and_then(OwnedArgument::as_strings);
        let Some(kinds) = kinds.filter(|kinds| kinds.len() == segments) else {
            problem!(
                None,
                WeldProblemKind::BadAttribute {
                    attribute: "weld.kind",
                    expected: Some(segments),
                },
            );
            done!();
        };
        let indices = node
            .attribute("weld.index")
            .and_then(OwnedArgument::as_i32s);
        let Some(indices) =
            indices.filter(|indices| indices.len() == 3 * segments)
        else {
            problem!(
                None,
                WeldProblemKind::BadAttribute {
                    attribute: "weld.index",
                    expected: Some(segments),
                },
            );
            done!();
        };
        let reverse = match node.attribute("weld.reverse") {
            None => vec![0; segments],
            Some(values) => match values.as_i32s() {
                Some(values) if values.len() == segments => values.to_vec(),
                _ => {
                    problem!(
                        None,
                        WeldProblemKind::BadAttribute {
                            attribute: "weld.reverse",
                            expected: Some(segments),
                        },
                    );
                    done!();
                }
            },
        };
        let ranges = match node.attribute("weld.range") {
            None => [0.0, 1.0].repeat(segments),
            Some(values) => match values.as_f32s() {
                Some(values) if values.len() == 2 * segments => values.to_vec(),
                _ => {
                    problem!(
                        None,
                        WeldProblemKind::BadAttribute {
                            attribute: "weld.range",
                            expected: Some(segments),
                        },
                    );
                    done!();
                }
            },
        };

        let boundaries = Boundaries::of(node);
        let mut first_segment = 0;
        for (use_index, (&id, &count)) in ids.iter().zip(&counts).enumerate() {
            let span = first_segment..first_segment + count;
            first_segment += count;

            let Ok(id) = u32::try_from(id) else {
                problem!(Some(use_index), WeldProblemKind::NegativeId { id });
                continue;
            };
            if count == 0 {
                problem!(Some(use_index), WeldProblemKind::EmptyUse);
                continue;
            }

            let mut chain = Vec::with_capacity(count);
            let mut invalid = None;
            for segment in span {
                let index = [
                    indices[3 * segment],
                    indices[3 * segment + 1],
                    indices[3 * segment + 2],
                ];
                let range = [ranges[2 * segment], ranges[2 * segment + 1]];
                let kind = match boundaries.select(&kinds[segment], index) {
                    Ok(kind) => kind,
                    Err(kind) => {
                        invalid = Some(kind);
                        break;
                    }
                };
                if !(0.0..=1.0).contains(&range[0])
                    || !(0.0..=1.0).contains(&range[1])
                    || range[0] >= range[1]
                {
                    invalid = Some(WeldProblemKind::BadRange { range });
                    break;
                }
                chain.push(WeldSegment {
                    kind,
                    reverse: reverse[segment] != 0,
                    range,
                });
            }
            if let Some(kind) = invalid {
                problem!(Some(use_index), kind);
                continue;
            }

            // A closed loop is a complete use.
            let partial_loop = chain.iter().any(|segment| {
                matches!(segment.kind, WeldKind::TrimLoop { .. })
                    && (1 < chain.len() || segment.range != [0.0, 1.0])
            });
            if partial_loop {
                problem!(Some(use_index), WeldProblemKind::PartialLoop);
                continue;
            }

            let mut unverified = false;
            let mut broken = None;
            for (position, pair) in chain.windows(2).enumerate() {
                match trim_curves_continue(&boundaries, &pair[0], &pair[1]) {
                    Some(true) => {}
                    Some(false) => {
                        broken = Some(position + 1);
                        break;
                    }
                    None => unverified = true,
                }
            }
            if let Some(segment) = broken {
                problem!(
                    Some(use_index),
                    WeldProblemKind::NotConnected { segment },
                );
                continue;
            }
            if unverified {
                problem!(
                    Some(use_index),
                    WeldProblemKind::ConnectivityUnverified,
                );
            }

            uses.push(WeldUse {
                geometry: handle,
                weld,
                id,
                use_index,
                segments: chain,
            });
        }
        WeldTable { uses, problems }
    }

    /// Every joined boundary in the scene, grouped by `(weld node, id)`,
    /// with every problem in every geometry's table.
    pub fn welds(&self) -> Welds<'_> {
        let mut welds = Welds::default();
        // A scene can hold tens of thousands of joins; find each group by
        // key rather than by scanning the groups found so far.
        let mut position = crate::HashMap::<(&str, u32), usize>::default();
        for (handle, _) in self.nodes() {
            let table = self.weld_table(handle);
            welds.problems.extend(table.problems);
            for weld_use in table.uses {
                let key = (weld_use.weld, weld_use.id);
                match position.get(&key) {
                    Some(&index) => welds.welds[index].uses.push(weld_use),
                    None => {
                        position.insert(key, welds.welds.len());
                        welds.welds.push(Weld {
                            weld: weld_use.weld,
                            id: weld_use.id,
                            uses: vec![weld_use],
                        });
                    }
                }
            }
        }
        welds
    }
}

#[cfg(test)]
mod tests;
