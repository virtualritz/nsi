//! STEP file to `monstertruck` compressed shells, each with its assembly
//! placement.
//!
//! Lifted from `monster-step-viewer`'s `src/step_loader.rs`
//! (`load_step_from_string_inner`), keeping only what loading needs: no
//! progress, streaming, colors, tessellation or retessellation.
use super::assembly::{Transform, parse_assembly_transforms};
use monstertruck::{
    modeling::Point3,
    step::load::{
        Table,
        ruststep::parser::parse,
        step_geometry::{Curve3D, StepParameterCurve, Surface},
    },
    topology::compress::CompressedTrimmedShell,
};
use nsi_procedural::Error;
use rayon::prelude::*;
use std::path::Path;

/// A shell as the STEP loader produces it: shared edges, and faces whose
/// boundaries are edge-uses referring to them by index.
pub type StepCompressedShell =
    CompressedTrimmedShell<Point3, Curve3D, Surface, StepParameterCurve>;

/// One shell of a STEP file.
pub struct LoadedShell {
    pub shell: StepCompressedShell,
    /// Where the assembly places the shell, if anywhere.
    pub placement: Option<Transform>,
}

/// Every shell of the STEP file at `path`, in entity order.
pub fn load_step_file(path: &Path) -> Result<Vec<LoadedShell>, Error> {
    let raw = std::fs::read_to_string(path).map_err(|error| {
        format!("failed to read STEP file {}: {error}", path.display())
    })?;
    let assembly_transforms = parse_assembly_transforms(&raw);

    let exchange = parse(&raw)
        .map_err(|error| format!("failed to parse STEP file: {error}"))?;
    let table = Table::from_data_section(
        exchange
            .data
            .first()
            .ok_or("STEP file has no data sections")?,
    );

    let mut shell_entries: Vec<_> = table.shell.iter().collect();
    shell_entries.sort_by_key(|(id, _)| *id);

    let shells = shell_entries
        .into_par_iter()
        .map(|(shell_id, shell_holder)| {
            let shell = table
                .to_compressed_trimmed_shell(shell_holder)
                .map_err(|error| {
                    format!(
                        "failed to convert STEP shell into topology: {error}"
                    )
                })?;
            Ok(LoadedShell {
                shell,
                placement: assembly_transforms.get(shell_id).copied(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;

    if shells.is_empty() {
        return Err("no shells found in STEP file".into());
    }
    Ok(shells)
}
