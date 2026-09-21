//! A procedural that loads a STEP file and emits its B-rep faces as
//! `nurbs` nodes, with weld declarations so a tessellator can keep the
//! edges the faces share watertight.
//!
//! Build it with `cargo build --example step_procedural`, then evaluate it
//! from any ɴsɪ scene:
//!
//! ```text
//! Evaluate
//!     "type" "string" 1 "dynamiclibrary"
//!     "filename" "string" 1 "target/debug/examples/libstep_procedural.so"
//!     "stepfilename" "string" 1 "part.stp"
//!     "weld" "int" 1 1
//! ```
//!
//! # Parameters
//!
//! * `stepfilename` (string, required) -- the STEP file. When the
//!   procedural is compiled in and called through
//!   [`nsi_procedural::execute`], `filename` is read too; through
//!   `NSIEvaluate`, `filename` is the library itself.
//! * `weld` (int, default 1) -- whether to declare weld tables.
//!
//! # What it emits
//!
//! Per shell `<stem>_shell<i>`, where `<stem>` is the STEP file's name
//! without extension:
//!
//! * a `transform` of that name, connected to `.root`, carrying the
//!   shell's assembly placement;
//! * per face a `nurbs` node `<stem>_shell<i>_face<j>` below it;
//! * with `weld` 1, a `weld` node `<stem>_shell<i>_welds`, the namespace
//!   of the shell's weld ids, connected to every face that declares one.
//!
//! # Weld declarations
//!
//! The weld id of a boundary edge-use is the index of its edge in the
//! shell's `edges`. `monstertruck`'s STEP loader numbers each STEP
//! `EDGE_CURVE` once per shell, and every face's edge-uses refer to their
//! edge by that number, so the two faces that share an edge declare the
//! same id.
//!
//! Each edge-use is one use, whose segments are the trim curves that
//! trace it, as `trim-curve` selectors in loop order. Trim curves the
//! conversion makes up -- a band's seam sides, a closing line -- trace no
//! edge and are declared by no use.
//!
//! A trim loop that merely traces the surface's domain is dropped, welded
//! or not: the patch's own domain says as much. Its edges are shared with
//! neighbours all the same, so with `weld` 1 each edge-use it traced is
//! declared as `nurbs-side` segments: the side it runs along, the part of
//! that side as `weld.range`, and its direction as `weld.reverse`.
//!
//! # Facing
//!
//! Every face's `∂P/∂u × ∂P/∂v` side faces out of the solid: the side
//! 3Delight 2.9.210 takes as the front, and the side `nsi-tessellate`
//! orients its meshes to. A one-sided shader shades the part from outside.

mod assembly;
mod brep;
pub mod loader;
mod sample;

use brep::{NsiBrepSurfaceData, NsiBrepTrimData, SideUse, TrimLoopPolicy};
use monstertruck::{
    step::load::step_geometry::Curve3D, topology::compress::CompressedEdge,
};
use nsi_ffi_wrap as nsi;
use nsi_procedural::{Error, Params, Procedural, Report};
use sample::FaceGeometry;
use std::{num::NonZeroUsize, path::Path};

/// When two points on a boundary count as one, in scene units: what
/// tells a closed edge from an open one.
const CLOSURE_TOLERANCE: f64 = 1.0e-4;

/// The STEP procedural.
pub struct StepProcedural;

/// A face's weld table, in the draft's layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeldTable {
    /// Per use, `weld.id`: the edge's index in the shell's `edges`.
    pub ids: Vec<i32>,
    /// Per use, `weld.segment-count`: how many segments trace it.
    pub segment_counts: Vec<i32>,
    /// Per segment, `weld.kind`: `trim-curve` or `nurbs-side`.
    pub kinds: Vec<&'static str>,
    /// Per segment, the first component of `weld.index`: the trim curve,
    /// counted over the flattened curves of all loops, or the side.
    pub indices: Vec<i32>,
    /// Per segment, `weld.reverse`.
    pub reverse: Vec<i32>,
    /// Per segment, `weld.range`.
    pub ranges: Vec<[f32; 2]>,
}

impl WeldTable {
    /// The table of a face with `trims` and natural `sides`: one use per
    /// boundary edge-use, its segments the trim curves or the parts of
    /// sides that trace it, in loop order.
    pub fn of(
        trims: Option<&NsiBrepTrimData>,
        sides: &[SideUse],
    ) -> Result<Self, Error> {
        let mut table = Self::default();
        let (ncurves, all_edge_uses) = trims
            .map_or((&[][..], &[][..]), |trims| {
                (&trims.ncurves[..], &trims.edge_uses[..])
            });
        let mut first = 0;
        for &count in ncurves {
            let count = usize::try_from(count)?;
            let edge_uses = &all_edge_uses[first..first + count];
            let at = |position: usize| edge_uses[position % count];
            // Start where a run of one edge-use begins, so a use whose
            // curves wrap around the loop's first curve stays one chain.
            let start = (0..count)
                .find(|&position| at(position) != at(position + count - 1))
                .unwrap_or(0);
            let mut position = 0;
            while position < count {
                let edge_use = at(start + position);
                let run = (position..count)
                    .take_while(|&next| at(start + next) == edge_use)
                    .count();
                if let Some(edge_use) = edge_use {
                    table.ids.push(i32::try_from(edge_use.edge)?);
                    table.segment_counts.push(i32::try_from(run)?);
                    for next in position..position + run {
                        table.push_segment(
                            "trim-curve",
                            i32::try_from(first + (start + next) % count)?,
                            false,
                            [0.0, 1.0],
                        );
                    }
                }
                position += run;
            }
            first += count;
        }
        for run in sides.chunk_by(|a, b| a.edge_use == b.edge_use) {
            table.ids.push(i32::try_from(run[0].edge_use.edge)?);
            table.segment_counts.push(i32::try_from(run.len())?);
            run.iter().for_each(|side| {
                table.push_segment(
                    "nurbs-side",
                    side.side,
                    side.reverse,
                    side.range,
                )
            });
        }
        Ok(table)
    }

    fn push_segment(
        &mut self,
        kind: &'static str,
        index: i32,
        reverse: bool,
        range: [f32; 2],
    ) {
        self.kinds.push(kind);
        self.indices.push(index);
        self.reverse.push(i32::from(reverse));
        self.ranges.push(range);
    }

    /// Sets `weld.reverse` on every use, against the reference traversal
    /// of its weld.
    ///
    /// That traversal is the shared edge's own curve: the one thing both
    /// faces have in common, and for a closed edge an anchor at its
    /// vertex rather than wherever a face's trim happens to start. A use
    /// that cannot follow it is dropped rather than declared.
    ///
    /// Without geometry -- a surface this cannot evaluate -- the uses are
    /// left as they were declared, which is honest: a wrong direction
    /// would fold the neighbouring face.
    fn declare_directions(
        &mut self,
        surface: &NsiBrepSurfaceData,
        geometry: Option<&FaceGeometry>,
        edges: &[CompressedEdge<Curve3D>],
    ) {
        let Some(geometry) = geometry else {
            return;
        };
        let mut kept = Self::default();
        let mut segment = 0;
        for (use_index, &id) in self.ids.iter().enumerate() {
            let count = self.segment_counts[use_index].max(0) as usize;
            let first = segment;
            segment += count;
            let keep = |table: &mut Self, reverse: Option<i32>| {
                table.ids.push(id);
                table.segment_counts.push(count as i32);
                for at in first..first + count {
                    table.kinds.push(self.kinds[at]);
                    table.indices.push(self.indices[at]);
                    table.reverse.push(reverse.unwrap_or(self.reverse[at]));
                    table.ranges.push(self.ranges[at]);
                }
            };

            // One segment is the measured case; a chain would need its
            // pieces sampled end to end, which no export here produces.
            if count != 1 {
                log::warn!(
                    "NSI BRep emitter: weld {id} has a {count}-segment use; \
                     its direction is left as declared"
                );
                keep(&mut kept, None);
                continue;
            }
            let traversal = match self.kinds[first] {
                "nurbs-side" => geometry.side(
                    surface,
                    self.indices[first],
                    self.ranges[first],
                ),
                _ => geometry.trim(
                    self.indices[first].max(0) as usize,
                    self.ranges[first],
                ),
            };
            let reference = usize::try_from(id)
                .ok()
                .and_then(|edge| edges.get(edge))
                .map(sample::edge);
            let (Some(traversal), Some(reference)) = (traversal, reference)
            else {
                keep(&mut kept, None);
                continue;
            };

            // A closed weld's uses may start at different points: the
            // contract prefers a shared anchor, which a periodic patch's
            // side cannot offer without a parameter search, and leaves
            // matching them to the renderer. Only the direction is
            // declared here.
            keep(
                &mut kept,
                Some(i32::from(
                    traversal.runs_against(&reference, CLOSURE_TOLERANCE),
                )),
            );
        }
        *self = kept;
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

impl StepProcedural {
    /// Emits one shell: its placement, its faces, and their welds.
    fn emit_shell<N>(
        nsi: &N,
        prefix: &str,
        shell: &loader::LoadedShell,
        weld: bool,
    ) -> Result<usize, Error>
    where
        for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
    {
        let policy = if weld {
            TrimLoopPolicy::NaturalSides
        } else {
            TrimLoopPolicy::DropFullDomain
        };
        let surfaces = brep::shell_to_nsi_surfaces(&shell.shell, policy);
        if surfaces.is_empty() {
            return Ok(0);
        }

        // The B-rep is in the shell's own coordinates; without its
        // placement an assembly's parts stack up at the origin.
        nsi.create(prefix, nsi::TRANSFORM, None)?;
        nsi.connect(prefix, None, nsi::ROOT, "objects", None)?;
        if let Some(placement) = shell.placement {
            nsi.set_attribute(
                prefix,
                &[nsi::matrix4_f64!(
                    "transformationmatrix",
                    &placement.to_nsi()
                )],
            )?;
        }

        let welds = format!("{prefix}_welds");
        if weld {
            nsi.create(&welds, "weld", None)?;
        }
        // Every use of a weld must follow one reference traversal. The
        // first use of each id sets it, in space; the rest say with
        // `weld.reverse` whether they run against it. A renderer cannot
        // work this out for a closed boundary, whose ends are one point,
        // so the exporter owes it the answer.

        for surface in &surfaces {
            let face = format!("{prefix}_face{}", surface.face_index);
            nsi.create(&face, nsi::NURBS, None)?;
            nsi.connect(&face, None, prefix, "objects", None)?;
            set_surface(nsi, &face, surface)?;
            if let Some(trims) = &surface.trims {
                set_trims(nsi, &face, trims)?;
            }
            if weld {
                let mut table =
                    WeldTable::of(surface.trims.as_ref(), &surface.sides)?;
                table.declare_directions(
                    surface,
                    FaceGeometry::of(surface, surface.trims.as_ref()).as_ref(),
                    &shell.shell.edges,
                );
                if !table.is_empty() {
                    nsi.connect(&welds, None, &face, "weld", None)?;
                    set_weld_table(nsi, &face, &table)?;
                }
            }
        }
        Ok(surfaces.len())
    }
}

impl Procedural for StepProcedural {
    fn load(_: &Report<'_>, _: &str) -> Result<Self, Error> {
        Ok(StepProcedural)
    }

    fn execute<N>(
        &self,
        nsi: &N,
        report: &Report<'_>,
        params: Params<'_>,
    ) -> Result<(), Error>
    where
        for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
    {
        let filename = params
            .get("stepfilename")
            .or_else(|| params.get("filename"))
            .and_then(|param| param.string())
            .ok_or("the `stepfilename` string parameter is required")?
            .to_str()
            .map_err(|_| "the STEP file name is not UTF-8")?;
        let weld = params.get("weld").and_then(|p| p.i32()).unwrap_or(1) != 0;

        let path = Path::new(filename);
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .unwrap_or("step");
        let shells = loader::load_step_file(path)?;

        let mut faces = 0;
        for (shell_index, shell) in shells.iter().enumerate() {
            let prefix = format!("{stem}_shell{shell_index}");
            faces += Self::emit_shell(nsi, &prefix, shell, weld)?;
        }
        report.info(&format!(
            "step_procedural: {} shell(s), {faces} face(s) from {filename}",
            shells.len()
        ));
        Ok(())
    }
}

fn set_surface<N>(
    nsi: &N,
    face: &str,
    surface: &NsiBrepSurfaceData,
) -> Result<(), Error>
where
    for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    nsi.set_attribute(
        face,
        &[
            nsi::integer_i32!("nu", surface.nu),
            nsi::integer_i32!("nv", surface.nv),
            nsi::integer_i32!("uorder", surface.uorder),
            nsi::integer_i32!("vorder", surface.vorder),
            nsi::real_f32_slice!("uknot", &surface.uknot),
            nsi::real_f32_slice!("vknot", &surface.vknot),
            nsi::real_f32!("umin", surface.umin),
            nsi::real_f32!("umax", surface.umax),
            nsi::real_f32!("vmin", surface.vmin),
            nsi::real_f32!("vmax", surface.vmax),
            nsi::point4_f32_slice!("Pw", &surface.pw),
        ],
    )?;
    Ok(())
}

fn set_trims<N>(
    nsi: &N,
    face: &str,
    trims: &NsiBrepTrimData,
) -> Result<(), Error>
where
    for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    nsi.set_attribute(
        face,
        &[
            nsi::integer_i32_slice!("trimcurves.ncurves", &trims.ncurves),
            nsi::integer_i32_slice!("trimcurves.n", &trims.n),
            nsi::integer_i32_slice!("trimcurves.order", &trims.order),
            nsi::real_f32_slice!("trimcurves.knot", &trims.knot),
            nsi::real_f32_slice!("trimcurves.min", &trims.min),
            nsi::real_f32_slice!("trimcurves.max", &trims.max),
            nsi::real_f32_slice!("trimcurves.u", &trims.u),
            nsi::real_f32_slice!("trimcurves.v", &trims.v),
            nsi::real_f32_slice!("trimcurves.w", &trims.w),
        ],
    )?;
    Ok(())
}

fn set_weld_table<N>(
    nsi: &N,
    face: &str,
    table: &WeldTable,
) -> Result<(), Error>
where
    for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
{
    let indices = table
        .indices
        .iter()
        .flat_map(|&index| [index, 0, 0])
        .collect::<Vec<_>>();
    let ranges = table.ranges.concat();
    nsi.set_attribute(
        face,
        &[
            nsi::integer_i32_slice!("weld.id", &table.ids),
            nsi::integer_i32_slice!(
                "weld.segment-count",
                &table.segment_counts
            ),
            nsi::string_slice!("weld.kind", &table.kinds),
            nsi::integer_i32_slice!("weld.index", &indices)
                // SAFETY: 3 is not zero, and a zero would fail to compile.
                .array_len(const { NonZeroUsize::new(3).unwrap() }),
            nsi::integer_i32_slice!("weld.reverse", &table.reverse),
            nsi::real_f32_slice!("weld.range", &ranges)
                // SAFETY: 2 is not zero, and a zero would fail to compile.
                .array_len(const { NonZeroUsize::new(2).unwrap() }),
        ],
    )?;
    Ok(())
}

nsi_procedural::declare_procedural!(StepProcedural);
