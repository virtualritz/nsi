//! The ɴsɪ [naming-convention draft]'s vocabulary, and the shipped names
//! it replaces.
//!
//! A [`Scene`](crate::Scene) stores the draft's names. A shipped name --
//! `nu` on a `nurbs` node, `nicename` on any -- is mapped on the way in
//! and warned about once, and mapped back on the way out, so a stream
//! written for the renderer of today still round-trips.
//!
//! The mapping is scoped by node type, as the draft's tables are: the
//! same word can mean different things on different nodes, and
//! `sourcemodels` renames to `objects` only on `instances`.
//!
//! What is deliberately *not* renamed is in spec 012: the ᴏsʟ globals
//! (`P`, `N`, `Pw` and kin), the two consolidating rows, and the API
//! arguments.
//!
//! [naming-convention draft]: https://nsi.readthedocs.io/en/latest/naming-convention.html

mod table;

use ahash::AHashMap as HashMap;
use parking_lot::Mutex;
use std::sync::OnceLock;

type Names = HashMap<(&'static str, &'static str), &'static str>;

/// The draft's name for each shipped one, by node type.
fn drafted() -> &'static Names {
    static DRAFTED: OnceLock<Names> = OnceLock::new();
    DRAFTED.get_or_init(|| {
        table::ATTRIBUTES
            .iter()
            .map(|&(node_type, legacy, draft)| ((node_type, legacy), draft))
            .collect()
    })
}

/// The shipped name for each of the draft's, by node type.
fn shipped() -> &'static Names {
    static SHIPPED: OnceLock<Names> = OnceLock::new();
    SHIPPED.get_or_init(|| {
        table::ATTRIBUTES
            .iter()
            .map(|&(node_type, legacy, draft)| ((node_type, draft), legacy))
            .collect()
    })
}

/// Looks `name` up for `node_type`, falling back to the rows that apply
/// to every node.
fn look_up(
    names: &'static Names,
    node_type: &str,
    name: &str,
) -> Option<&'static str> {
    // The table is keyed by the shipped node type, and a `Scene` holds
    // the draft's: `output-layer` asks under `outputlayer`.
    let node_type = legacy_node_type(node_type).unwrap_or(node_type);
    names
        .get(&(node_type, name))
        .or_else(|| names.get(&("", name)))
        .copied()
}

/// The draft's name for the shipped `name` on a node of `node_type`, if
/// the draft renames it.
pub fn draft_attribute(node_type: &str, name: &str) -> Option<&'static str> {
    look_up(drafted(), node_type, name)
}

/// The shipped name for the draft's `name` on a node of `node_type`, if
/// the draft renamed it.
pub fn legacy_attribute(node_type: &str, name: &str) -> Option<&'static str> {
    look_up(shipped(), node_type, name)
}

/// The draft's name for the shipped node type `node_type`, if the draft
/// renames it.
pub fn draft_node_type(node_type: &str) -> Option<&'static str> {
    table::NODE_TYPES
        .iter()
        .find(|&&(legacy, _)| legacy == node_type)
        .map(|&(_, draft)| draft)
}

/// The shipped name for the draft's node type `node_type`, if the draft
/// renamed it.
pub fn legacy_node_type(node_type: &str) -> Option<&'static str> {
    table::NODE_TYPES
        .iter()
        .find(|&&(_, draft)| draft == node_type)
        .map(|&(legacy, _)| legacy)
}

/// The node type whose rows apply to `handle`, for the reserved nodes
/// that are never created and so carry no type of their own.
pub(crate) fn reserved_node_type(handle: &str) -> Option<&'static str> {
    match handle {
        crate::GLOBAL => Some("global"),
        crate::ROOT => Some("root"),
        _ => None,
    }
}

/// Renames `argument` to the draft's vocabulary, warning once when it
/// carried a shipped name.
pub(crate) fn to_draft(node_type: &str, argument: &mut crate::OwnedArgument) {
    if let Some(draft) = draft_attribute(node_type, &argument.name) {
        warn_deprecated(&argument.name, draft);
        argument.name = draft.to_string();
    }
}

/// Warns that `legacy` is deprecated, once per name per process: the
/// name is what an exporter has to change, not the occurrence.
pub(crate) fn warn_deprecated(legacy: &str, draft: &str) {
    static WARNED: OnceLock<Mutex<ahash::AHashSet<String>>> = OnceLock::new();
    if WARNED
        .get_or_init(Default::default)
        .lock()
        .insert(legacy.to_string())
    {
        log::warn!("ɴsɪ `{legacy}` is deprecated; use `{draft}`");
    }
}

#[cfg(test)]
mod tests;
