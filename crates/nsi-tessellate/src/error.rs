//! What can go wrong turning a cage into triangles.

use core::fmt;

/// Why a primitive could not be tessellated.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The scene does not have that node.
    UnknownHandle {
        /// The handle asked for.
        handle: String,
    },
    /// The node is not a subdivision surface.
    ///
    /// A `mesh` without `subdivision.scheme` is the geometry, not a
    /// cage for one, so refining it would answer a question the scene
    /// did not ask.
    NotASubdivisionSurface {
        /// The node.
        handle: String,
    },
    /// ɴsɪ names a subdivision scheme this crate does not implement.
    UnknownScheme {
        /// The node.
        handle: String,
        /// What it asked for.
        scheme: String,
    },
    /// The cage has holes.
    ///
    /// ɴsɪ's `nholes` describes a face with interior perimeters, which
    /// a subdivision cage has no representation for: the kernel's
    /// faces are simple rings. Refusing beats refining the outer
    /// perimeter and quietly filling the hole.
    HolesInCage {
        /// The node.
        handle: String,
    },
    /// Resolving the scene failed.
    Resolve(nsi_intermediate::ResolveError),
    /// The subdivision kernel refused the cage.
    Kernel(subdiv_kernels::KernelError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownHandle { handle } => {
                write!(f, "the scene has no node {handle:?}")
            }
            Self::NotASubdivisionSurface { handle } => write!(
                f,
                "ɴsɪ node {handle:?} sets no \"subdivision.scheme\", so it \
                 is the geometry rather than a cage for one"
            ),
            Self::UnknownScheme { handle, scheme } => write!(
                f,
                "ɴsɪ node {handle:?} asks for the {scheme:?} subdivision \
                 scheme, which this crate does not implement"
            ),
            Self::HolesInCage { handle } => write!(
                f,
                "ɴsɪ node {handle:?} is a subdivision cage with \"nholes\"; \
                 a cage face is a simple ring"
            ),
            Self::Resolve(error) => error.fmt(f),
            Self::Kernel(error) => write!(f, "{error:?}"),
        }
    }
}

impl core::error::Error for Error {}

impl From<nsi_intermediate::ResolveError> for Error {
    fn from(error: nsi_intermediate::ResolveError) -> Self {
        Self::Resolve(error)
    }
}

impl From<subdiv_kernels::KernelError> for Error {
    fn from(error: subdiv_kernels::KernelError) -> Self {
        Self::Kernel(error)
    }
}
