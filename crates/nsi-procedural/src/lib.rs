//! Write [ɴsɪ](https://nsi.readthedocs.io/) procedurals in safe Rust.
//!
//! A procedural is code the renderer runs to contribute to the scene: a
//! dynamic library exporting `NSIProceduralLoad`, handed a context to
//! make ɴsɪ calls on. This crate turns that into a trait:
//!
//! 1. Implement [`Procedural`] for a type of your own.
//! 2. Invoke [`declare_procedural!`] once, at the crate root, to export
//!    the entry point.
//! 3. Build the crate as a `cdylib`.
//!
//! # A complete procedural
//!
//! ```
//! use nsi_ffi_wrap as nsi;
//! use nsi_procedural::{Error, Params, Procedural, Report};
//!
//! struct Spheres;
//!
//! impl Procedural for Spheres {
//!     fn load(_: &Report<'_>, _: &str) -> Result<Self, Error> {
//!         Ok(Spheres)
//!     }
//!
//!     fn execute<N>(
//!         &self,
//!         nsi: &N,
//!         _: &Report<'_>,
//!         params: Params<'_>,
//!     ) -> Result<(), Error>
//!     where
//!         for<'call> N: nsi::Nsi<Arg<'call> = nsi::Arg<'call, 'static>>,
//!     {
//!         let count = params.get("count").and_then(|p| p.i32()).unwrap_or(1);
//!         for index in 0..count {
//!             let handle = format!("sphere{index}");
//!             nsi.create(&handle, nsi::PARTICLES, None)?;
//!             nsi.connect(&handle, None, nsi::ROOT, "objects", None)?;
//!         }
//!         Ok(())
//!     }
//! }
//!
//! nsi_procedural::declare_procedural!(Spheres);
//! ```
//!
//! # Two ways in, one trait
//!
//! **Standalone.** Built as a `cdylib`, the procedural is loaded by any
//! ɴsɪ renderer through the C entry point -- `NSIEvaluate` with
//! `"type" "dynamiclibrary"`, or a `procedural` node. Rust has no stable
//! ABI, so a library loaded at runtime can only be spoken to through C.
//! Its calls go through the ɴsɪ implementation that loaded it, which
//! the renderer names when it does (`nsi.pdf` §2.7.1) -- not whichever
//! library this process would load by default.
//!
//! **Compiled in.** A renderer written against [`Nsi`](https://docs.rs/nsi-trait/latest/nsi_trait/trait.Nsi.html)
//! -- one built on `nsi-intermediate`, say -- links the procedural's
//! crate and calls it with [`execute`], passing its own `Nsi`
//! implementation. The calls stay Rust: no C strings, no parameter
//! marshalling.
//!
//! [`Procedural::execute`] is generic over the context, so the same
//! code serves both.
//!
//! # Errors and panics
//!
//! Both are reported through the renderer, at error level, and the
//! render goes on. Neither unwinds into C.
//!
//! # `link_lib3delight`
//!
//! Do not build a procedural with `nsi-ffi-wrap`'s `link_lib3delight`
//! feature. It resolves ɴsɪ at build time and refuses any other library,
//! so the procedural could not make its calls through the renderer
//! that loaded it.

mod params;
mod shim;

pub use params::{Param, Params};
pub use shim::{Error, Procedural, execute};

#[doc(hidden)]
pub use shim::{Descriptor, NSIReport, shim_load};

/// How serious a reported message is -- `NSIErrorLevel`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Level {
    /// Printed as is.
    Message = 0,
    /// Informational.
    Info = 1,
    /// Something is probably wrong.
    Warning = 2,
    /// Something is wrong.
    Error = 3,
}

/// Where a procedural sends messages: the renderer's own error
/// reporting, so they appear alongside the renderer's.
#[derive(Copy, Clone)]
pub struct Report<'a> {
    sink: &'a (dyn Fn(Level, &str) + Sync),
}

impl<'a> Report<'a> {
    /// A report that hands every message to `sink`. What a renderer
    /// calling [`execute`] passes; a standalone procedural is given one.
    pub fn new(sink: &'a (dyn Fn(Level, &str) + Sync)) -> Self {
        Self { sink }
    }

    /// Reports `message` at `level`.
    pub fn message(&self, level: Level, message: &str) {
        (self.sink)(level, message)
    }

    /// Reports `message` as information.
    pub fn info(&self, message: &str) {
        self.message(Level::Info, message)
    }

    /// Reports `message` as a warning.
    pub fn warning(&self, message: &str) {
        self.message(Level::Warning, message)
    }

    /// Reports `message` as an error.
    pub fn error(&self, message: &str) {
        self.message(Level::Error, message)
    }
}

/// Exports `$procedural` as an ɴsɪ procedural.
///
/// Emits `NSIProceduralLoad`, the one symbol the renderer resolves by
/// name. Invoke once, at the crate root of a `cdylib`:
///
/// ```ignore
/// nsi_procedural::declare_procedural!(MyProcedural);
/// ```
#[macro_export]
macro_rules! declare_procedural {
    ($procedural:ty) => {
        /// # Safety
        /// Called by the renderer per `nsi.pdf` §2.7.1.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn NSIProceduralLoad(
            context: ::core::ffi::c_int,
            report: $crate::NSIReport,
            nsi_library_path: *const ::core::ffi::c_char,
            renderer_version: *const ::core::ffi::c_char,
        ) -> *mut $crate::Descriptor {
            unsafe {
                $crate::shim_load::<$procedural>(
                    context,
                    report,
                    nsi_library_path,
                    renderer_version,
                )
            }
        }
    };
}

#[cfg(test)]
mod tests;
