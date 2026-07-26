//! GPU emission -- the seam a backend plugs into.
//!
//! Requirement R4 was resolved on 2026-07-26: **GLSL 4.60 is the GPU source
//! of record**, and SPIR-V passthrough is sufficient for v1 because the
//! realtime backend is Vulkan-first (feature 001) and `wgpu` accepts SPIR-V
//! passthrough on Vulkan. Compiling the assembled module to SPIR-V --
//! through glslang, `shaderc`, or whatever a backend already links -- is a
//! *backend build step* behind [`GpuEmitter`], so this crate takes no
//! compiler-toolchain dependency and no backend inherits one it does not
//! want.
//!
//! The trait is also how a second target arrives without touching a single
//! [`NodeDef`](crate::node::NodeDef): a WGSL emitter, or a native emitter
//! for a renderer with its own IR, implements [`GpuEmitter`] against the
//! same [`NetworkModule`].
//!
//! [`GlslPassthroughEmitter`] is the built-in: it hands back the assembled
//! GLSL and the entry-point name, which is exactly what a `shaderc` call
//! needs.
use crate::{error::Error, translate::NetworkModule};

/// The language of an [`EmittedShader`]'s code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderLanguage {
    /// GLSL 4.60 source, UTF-8.
    Glsl460,
    /// SPIR-V words, little-endian.
    SpirV,
    /// WGSL source, UTF-8.
    Wgsl,
}

/// What a [`GpuEmitter`] produces.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmittedShader {
    /// The function a backend dispatches.
    pub entry_point: String,
    /// What [`code`](Self::code) is.
    pub language: ShaderLanguage,
    /// The shader code.
    pub code: Vec<u8>,
}

impl EmittedShader {
    /// The code as text, for the textual languages.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        match self.language {
            ShaderLanguage::Glsl460 | ShaderLanguage::Wgsl => {
                core::str::from_utf8(&self.code).ok()
            }
            ShaderLanguage::SpirV => None,
        }
    }
}

/// Turns a translated network into something a GPU backend can consume.
pub trait GpuEmitter {
    /// Emits the shader for `module`.
    ///
    /// # Errors
    ///
    /// Implementations report their own failures -- an unsupported closure
    /// in the module's signature, a compiler diagnostic -- as [`Error`]
    /// values. Emitting must never silently drop part of a network.
    fn emit(&self, module: &NetworkModule) -> Result<EmittedShader, Error>;
}

/// The built-in emitter: the assembled GLSL, verbatim.
///
/// Backends that own a shader compiler take this output and compile it;
/// backends that consume GLSL directly use it as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GlslPassthroughEmitter;

impl GpuEmitter for GlslPassthroughEmitter {
    fn emit(&self, module: &NetworkModule) -> Result<EmittedShader, Error> {
        Ok(EmittedShader {
            entry_point: module.entry_point().to_string(),
            language: ShaderLanguage::Glsl460,
            code: module.glsl_source().as_bytes().to_vec(),
        })
    }
}
