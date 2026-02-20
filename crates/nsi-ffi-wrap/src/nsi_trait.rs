//! Core ɴsɪ trait for renderer implementations within nsi-ffi-wrap.
//!
//! This module provides the FFI-compatible `Nsi` trait and `NodeType` enum.
//! `Action` is re-exported from the `nsi-trait` crate.

use crate::ArgSlice;

// Re-export Action from nsi-trait crate - single source of truth
pub use ::nsi_trait::Action;

/// Node types in the ɴsɪ scene graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NodeType {
    All,
    Root,
    Global,
    Set,
    Shader,
    Attributes,
    Transform,
    Instances,
    Plane,
    Mesh,
    FaceSet,
    Curves,
    Particles,
    Procedural,
    Volume,
    Environment,
    OrthographicCamera,
    PerspectiveCamera,
    FisheyeCamera,
    CylindricalCamera,
    SphericalCamera,
    OutputDriver,
    OutputLayer,
    Screen,
}

impl NodeType {
    /// Returns the string identifier used by the C API.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::All => ".all",
            Self::Root => ".root",
            Self::Global => ".global",
            Self::Set => "set",
            Self::Shader => "shader",
            Self::Attributes => "attributes",
            Self::Transform => "transform",
            Self::Instances => "instances",
            Self::Plane => "plane",
            Self::Mesh => "mesh",
            Self::FaceSet => "faceset",
            Self::Curves => "curves",
            Self::Particles => "particles",
            Self::Procedural => "procedural",
            Self::Volume => "volume",
            Self::Environment => "environment",
            Self::OrthographicCamera => "orthographiccamera",
            Self::PerspectiveCamera => "perspectivecamera",
            Self::FisheyeCamera => "fisheyecamera",
            Self::CylindricalCamera => "cylindricalcamera",
            Self::SphericalCamera => "sphericalcamera",
            Self::OutputDriver => "outputdriver",
            Self::OutputLayer => "outputlayer",
            Self::Screen => "screen",
        }
    }

    /// Parse a node type from its string identifier.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            ".all" => Self::All,
            ".root" => Self::Root,
            ".global" => Self::Global,
            "set" => Self::Set,
            "shader" => Self::Shader,
            "attributes" => Self::Attributes,
            "transform" => Self::Transform,
            "instances" => Self::Instances,
            "plane" => Self::Plane,
            "mesh" => Self::Mesh,
            "faceset" => Self::FaceSet,
            "curves" => Self::Curves,
            "particles" => Self::Particles,
            "procedural" => Self::Procedural,
            "volume" => Self::Volume,
            "environment" => Self::Environment,
            "orthographiccamera" => Self::OrthographicCamera,
            "perspectivecamera" => Self::PerspectiveCamera,
            "fisheyecamera" => Self::FisheyeCamera,
            "cylindricalcamera" => Self::CylindricalCamera,
            "sphericalcamera" => Self::SphericalCamera,
            "outputdriver" => Self::OutputDriver,
            "outputlayer" => Self::OutputLayer,
            "screen" => Self::Screen,
            _ => return None,
        })
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Core ɴsɪ interface trait for FFI-compatible renderers.
pub trait Nsi: Send + Sync {
    type Handle: Clone + Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    fn begin(
        &self,
        args: Option<&ArgSlice>,
    ) -> Result<Self::Handle, Self::Error>;
    fn end(&self, handle: &Self::Handle) -> Result<(), Self::Error>;

    fn create(
        &self,
        ctx: &Self::Handle,
        handle: &str,
        node_type: NodeType,
        args: Option<&ArgSlice>,
    ) -> Result<(), Self::Error>;

    fn delete(
        &self,
        ctx: &Self::Handle,
        handle: &str,
        args: Option<&ArgSlice>,
    ) -> Result<(), Self::Error>;

    fn set_attribute(
        &self,
        ctx: &Self::Handle,
        handle: &str,
        args: &ArgSlice,
    ) -> Result<(), Self::Error>;

    fn set_attribute_at_time(
        &self,
        ctx: &Self::Handle,
        handle: &str,
        time: f64,
        args: &ArgSlice,
    ) -> Result<(), Self::Error>;

    fn delete_attribute(
        &self,
        ctx: &Self::Handle,
        handle: &str,
        name: &str,
    ) -> Result<(), Self::Error>;

    fn connect(
        &self,
        ctx: &Self::Handle,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Option<&ArgSlice>,
    ) -> Result<(), Self::Error>;

    fn disconnect(
        &self,
        ctx: &Self::Handle,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), Self::Error>;

    fn evaluate(
        &self,
        ctx: &Self::Handle,
        args: Option<&ArgSlice>,
    ) -> Result<(), Self::Error>;

    fn render_control(
        &self,
        ctx: &Self::Handle,
        action: Action,
        args: Option<&ArgSlice>,
    ) -> Result<(), Self::Error>;
}

/// Extension trait for convenience methods.
pub trait NsiExt: Nsi {
    fn render(
        &self,
        ctx: &Self::Handle,
        args: Option<&ArgSlice>,
    ) -> Result<(), Self::Error> {
        self.render_control(ctx, Action::Start, args)?;
        self.render_control(ctx, Action::Wait, None)
    }
}

impl<T: Nsi> NsiExt for T {}
