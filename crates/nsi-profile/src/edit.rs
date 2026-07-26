//! Edit classification -- what an edit costs.
//!
//! ɴsɪ distinguishes cheap, frequent attribute edits from structural
//! transactions. The profile applies the same split to materials
//! (`research.md` D4, `data-model.md` translation pipeline):
//!
//! - A parameter the [`ParameterBlock`](crate::parameter_block) carries can
//!   be animated by patching bytes -- [`EditClass::ParameterUpdate`], no
//!   re-translation, off the frame budget.
//! - Everything else -- topology, `shaderfilename`, and parameters baked
//!   into the module such as file names and enumerants -- is
//!   [`EditClass::Retranslate`].
//!
//! Classification is conservative by construction: anything not provably a
//! block field re-translates. There is no third "probably fine" answer.
use crate::translate::NetworkModule;

/// A scene edit, in the vocabulary of the ɴsɪ calls that would perform it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Edit {
    /// `NSISetAttribute` on a shader parameter.
    SetParam {
        /// The `shader` node handle.
        handle: String,
        /// The parameter name.
        param: String,
    },
    /// `NSIConnect` between two shader ports.
    Connect {
        /// Handle of the upstream node.
        from_handle: String,
        /// Output port on the upstream node.
        from_output: String,
        /// Handle of the downstream node.
        to_handle: String,
        /// Input port on the downstream node.
        to_input: String,
    },
    /// `NSIDisconnect` between two shader ports.
    Disconnect {
        /// Handle of the upstream node.
        from_handle: String,
        /// Output port on the upstream node.
        from_output: String,
        /// Handle of the downstream node.
        to_handle: String,
        /// Input port on the downstream node.
        to_input: String,
    },
    /// `NSICreate` of a `shader` node.
    CreateNode {
        /// The new node's handle.
        handle: String,
    },
    /// `NSIDelete` of a `shader` node.
    DeleteNode {
        /// The deleted node's handle.
        handle: String,
    },
    /// `NSISetAttribute` on `shaderfilename` -- the node becomes a different
    /// profile node, or leaves the profile entirely.
    SetShaderfilename {
        /// The `shader` node handle.
        handle: String,
    },
}

/// What an [`Edit`] costs against an already-translated network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditClass {
    /// Patch the parameter buffer in place; the module stays valid.
    ParameterUpdate {
        /// Byte offset of the field in the parameter block.
        offset: usize,
        /// Field size in bytes.
        size: usize,
    },
    /// Re-run [`translate`](crate::translate::translate).
    Retranslate,
}

/// Classifies an edit against a translated network.
///
/// Only [`Edit::SetParam`] on a parameter the block carries avoids
/// re-translation. A `SetParam` on a string parameter -- an
/// [`image`](crate::v1::IMAGE) file name, a `math_color` operation -- is a
/// [`Retranslate`](EditClass::Retranslate), because those values are baked
/// into the module as texture indices and enumerants.
#[must_use]
pub fn classify(edit: &Edit, module: &NetworkModule) -> EditClass {
    match edit {
        Edit::SetParam { handle, param } => {
            module.parameter_block().field(handle, param).map_or(
                EditClass::Retranslate,
                |field| EditClass::ParameterUpdate {
                    offset: field.offset,
                    size: field.size,
                },
            )
        }
        Edit::Connect { .. }
        | Edit::Disconnect { .. }
        | Edit::CreateNode { .. }
        | Edit::DeleteNode { .. }
        | Edit::SetShaderfilename { .. } => EditClass::Retranslate,
    }
}
