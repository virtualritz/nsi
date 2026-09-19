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
//! With `weld` 1, trim loops that merely trace the surface's domain are
//! kept: their curves are edges a neighbour shares.
//!
//! # Facing
//!
//! The B-rep conversion keeps `monster-step-viewer`'s orientation and
//! v-axis conventions, which leave every face's `du × dv` side facing into
//! the solid. 3Delight 2.9.210 takes that side as the front, so a
//! one-sided shader -- `dlConstant` with its default `doublesided` 0 --
//! shades the part black from outside.

mod assembly;
mod brep;
pub mod loader;

use brep::{NsiBrepSurfaceData, NsiBrepTrimData, TrimLoopPolicy};
use nsi_ffi_wrap as nsi;
use nsi_procedural::{Error, Params, Procedural, Report};
use std::{num::NonZeroUsize, path::Path};

/// The STEP procedural.
pub struct StepProcedural;

/// A face's weld table, in the draft's layout.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeldTable {
    /// Per use, `weld.id`: the edge's index in the shell's `edges`.
    pub ids: Vec<i32>,
    /// Per use, `weld.segment-count`: how many trim curves trace it.
    pub segment_counts: Vec<i32>,
    /// Per segment, the trim curve it selects, counted over the
    /// flattened curves of all loops.
    pub curves: Vec<i32>,
}

impl WeldTable {
    /// The table of a face with `trims`: one use per boundary edge-use,
    /// its segments the curves that trace it, in loop order.
    pub fn of(trims: &NsiBrepTrimData) -> Result<Self, Error> {
        let mut table = Self::default();
        let mut first = 0;
        for &count in &trims.ncurves {
            let count = usize::try_from(count)?;
            let edge_uses = &trims.edge_uses[first..first + count];
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
                        table.curves.push(i32::try_from(
                            first + (start + next) % count,
                        )?);
                    }
                }
                position += run;
            }
            first += count;
        }
        Ok(table)
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
            TrimLoopPolicy::KeepAll
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
        for surface in &surfaces {
            let face = format!("{prefix}_face{}", surface.face_index);
            nsi.create(&face, nsi::NURBS, None)?;
            nsi.connect(&face, None, prefix, "objects", None)?;
            set_surface(nsi, &face, surface)?;
            if let Some(trims) = &surface.trims {
                set_trims(nsi, &face, trims)?;
                if weld {
                    let table = WeldTable::of(trims)?;
                    if !table.is_empty() {
                        nsi.connect(&welds, None, &face, "weld", None)?;
                        set_weld_table(nsi, &face, &table)?;
                    }
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
    let kinds = vec!["trim-curve"; table.curves.len()];
    let indices = table
        .curves
        .iter()
        .flat_map(|&curve| [curve, 0, 0])
        .collect::<Vec<_>>();
    nsi.set_attribute(
        face,
        &[
            nsi::integer_i32_slice!("weld.id", &table.ids),
            nsi::integer_i32_slice!(
                "weld.segment-count",
                &table.segment_counts
            ),
            nsi::string_slice!("weld.kind", &kinds),
            nsi::integer_i32_slice!("weld.index", &indices)
                // SAFETY: 3 is not zero, and a zero would fail to compile.
                .array_len(const { NonZeroUsize::new(3).unwrap() }),
        ],
    )?;
    Ok(())
}

nsi_procedural::declare_procedural!(StepProcedural);
