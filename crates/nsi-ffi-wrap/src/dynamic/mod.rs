use crate::{FfiApi, *};
use dlopen2::wrapper::{Container, WrapperApi};
use crate::backend;
use std::{error::Error, ffi::c_char, os::raw::c_int, path::Path};

pub type ApiImpl = DynamicApi;

#[derive(WrapperApi)]
struct NsiCApi {
    NSIBegin:
        extern "C" fn(nparams: c_int, params: *const NSIParam) -> NSIContext,
    NSIEnd: extern "C" fn(ctx: NSIContext),
    NSICreate: extern "C" fn(
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSIDelete: extern "C" fn(
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSISetAttribute: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSISetAttributeAtTime: extern "C" fn(
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSIDeleteAttribute:
        extern "C" fn(ctx: NSIContext, object: NSIHandle, name: *const c_char),
    #[allow(clippy::too_many_arguments)]
    NSIConnect: extern "C" fn(
        ctx: NSIContext,
        from: NSIHandle,
        from_attribute: *const c_char,
        to: NSIHandle,
        to_attribute: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ),
    NSIDisconnect: extern "C" fn(
        ctx: NSIContext,
        from: NSIHandle,
        from_attribute: *const c_char,
        to: NSIHandle,
        to_attribute: *const c_char,
    ),
    NSIEvaluate:
        extern "C" fn(ctx: NSIContext, nparams: c_int, params: *const NSIParam),
    NSIRenderControl:
        extern "C" fn(ctx: NSIContext, nparams: c_int, params: *const NSIParam),
    #[cfg(feature = "output")]
    DspyRegisterDriver: extern "C" fn(
        driver_name: *const c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError,
}

pub struct DynamicApi {
    api: Container<NsiCApi>,
}

impl DynamicApi {
    /// The renderer a name refers to.
    ///
    /// A name in [`backend::KNOWN`] is searched for where its vendor
    /// installs; anything else is treated as a library to load. See
    /// [`backend`] for the whole rule and why it is that way round.
    ///
    /// **Every candidate is tried before failing**, and the error
    /// names all of them. A renderer that cannot be found is the one
    /// failure this crate cannot render through, so the message has to
    /// carry enough to fix it without a second run.
    pub fn load(name: &str) -> Result<Self, LoadError> {
        let candidates = backend::candidates(name);
        let mut tried = Vec::with_capacity(candidates.len());

        for path in candidates {
            // SAFETY: `Container::load` is `dlopen`, which is safe to
            // call with any path; a path that is not a library is an
            // error rather than undefined behaviour. The symbols
            // `NsiCApi` names are the ɴsɪ C API, whose signatures are
            // fixed by the specification -- so a library that resolves
            // them at all resolves them with these types.
            match unsafe { Container::load(&path) } {
                Ok(api) => {
                    let api = DynamicApi { api };

                    #[cfg(feature = "output")]
                    super::register_output_drivers(&api);

                    return Ok(api);
                }
                Err(error) => tried.push((path, error.to_string())),
            }
        }

        Err(LoadError {
            name: std::string::String::from(name),
            known: backend::lookup(name).is_some(),
            tried,
        })
    }
}

/// No ɴsɪ renderer could be loaded for a name.
#[derive(Debug)]
pub struct LoadError {
    /// What was asked for.
    name: std::string::String,
    /// Whether it was a name this crate knows, which decides what the
    /// message can usefully suggest.
    known: bool,
    /// Every path tried, and why each failed.
    tried: Vec<(std::path::PathBuf, std::string::String)>,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { name, known, tried } = self;

        if *known {
            let prefix_var = backend::lookup(name.as_str())
                .map(|backend| backend.prefix_var)
                .unwrap_or_default();
            write!(
                f,
                "the ɴsɪ renderer {name:?} is known but its library could \
                 not be loaded; set ${prefix_var} to its install prefix, \
                 or pass the library's path as the \"renderer\" argument"
            )?;
        } else {
            let known: Vec<&str> = backend::known_names().collect();
            write!(
                f,
                "no ɴsɪ renderer is named {name:?}, and no library by that \
                 name could be loaded either; the names this build knows \
                 are {}, and anything else is taken as a library to open",
                known.join(", ")
            )?;
        }

        writeln!(f, ". Tried:")?;
        for (path, why) in tried {
            writeln!(f, "  {} -- {why}", path.display())?;
        }
        Ok(())
    }
}

impl Error for LoadError {}

impl TryFrom<&Path> for DynamicApi {
    type Error = dlopen2::Error;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        // SAFETY: as in `load` above.
        match unsafe { Container::load(path) } {
            Err(e) => Err(e),
            Ok(api) => {
                let api = DynamicApi { api };

                #[cfg(feature = "output")]
                super::register_output_drivers(&api);

                Ok(api)
            }
        }
    }
}

impl FfiApi for DynamicApi {
    #[inline]
    fn NSIBegin(&self, nparams: c_int, params: *const NSIParam) -> NSIContext {
        self.api.NSIBegin(nparams, params)
    }

    #[inline]
    fn NSIEnd(&self, ctx: NSIContext) {
        self.api.NSIEnd(ctx);
    }

    #[inline]
    fn NSICreate(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        type_: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSICreate(ctx, handle, type_, nparams, params);
    }

    #[inline]
    fn NSIDelete(
        &self,
        ctx: NSIContext,
        handle: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIDelete(ctx, handle, nparams, params);
    }

    #[inline]
    fn NSISetAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSISetAttribute(ctx, object, nparams, params);
    }

    #[inline]
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        time: f64,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api
            .NSISetAttributeAtTime(ctx, object, time, nparams, params);
    }

    #[inline]
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext,
        object: NSIHandle,
        name: *const c_char,
    ) {
        self.api.NSIDeleteAttribute(ctx, object, name);
    }

    #[inline]
    fn NSIConnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attribute: *const c_char,
        to: NSIHandle,
        to_attribute: *const c_char,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIConnect(
            ctx,
            from,
            from_attribute,
            to,
            to_attribute,
            nparams,
            params,
        );
    }

    #[inline]
    fn NSIDisconnect(
        &self,
        ctx: NSIContext,
        from: NSIHandle,
        from_attribute: *const c_char,
        to: NSIHandle,
        to_attribute: *const c_char,
    ) {
        self.api
            .NSIDisconnect(ctx, from, from_attribute, to, to_attribute);
    }

    #[inline]
    fn NSIEvaluate(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIEvaluate(ctx, nparams, params);
    }

    #[inline]
    fn NSIRenderControl(
        &self,
        ctx: NSIContext,
        nparams: c_int,
        params: *const NSIParam,
    ) {
        self.api.NSIRenderControl(ctx, nparams, params);
    }

    #[cfg(feature = "output")]
    #[inline]
    fn DspyRegisterDriver(
        &self,
        driver_name: *const c_char,
        p_open: ndspy_sys::PtDspyOpenFuncPtr,
        p_write: ndspy_sys::PtDspyWriteFuncPtr,
        p_close: ndspy_sys::PtDspyCloseFuncPtr,
        p_query: ndspy_sys::PtDspyQueryFuncPtr,
    ) -> ndspy_sys::PtDspyError {
        self.api.DspyRegisterDriver(
            driver_name,
            p_open,
            p_write,
            p_close,
            p_query,
        )
    }
}
