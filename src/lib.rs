//! A flexible, modern API for offline 3D renderers
//!
//! The [Nodal Scene Interface](https://nsi.readthedocs.io/) (ɴsɪ) is
//! built around the concept of nodes.
//!
//! Each node has a unique handle to identify it. It also has a type
//! which describes its intended function in the scene.
//!
//! Nodes are abstract containers for data. The interpretation depends
//! on the node type. Nodes can also be [connected to each
//! other](https://nsi.readthedocs.io/en/latest/guidelines.html#basic-scene-anatomy)
//! to express relationships.
//!
//! Data is stored on nodes as attributes. Each attribute has a name
//! which is unique on the node and a type which describes the kind of
//! data it holds (strings, integer numbers, floating point numbers,
//! etc).
//!
//! Relationships and data flow between nodes are represented as
//! connections. Connections have a source and a destination. Both can
//! be either a node or a specific attribute of a node. There are no
//! type restrictions for connections in the interface itself. It is
//! acceptable to connect attributes of different types or even
//! attributes to nodes. The validity of such connections depends on
//! the types of the nodes involved.
//!
//! What we refer to as the ɴsɪ has two major components:
//!
//! 1.  Methods to create nodes, attributes and their connections.
//!     These are attached to a rendering [`Context`].
//!
//! 2.  Nodes of different [`NodeType`]s understood by the renderer.
//!
//! Much of the complexity and expressiveness of the interface comes
//! from
//! [the supported nodes](https://nsi.readthedocs.io/en/latest/nodes.html).
//!
//! The first part was kept deliberately simple to make it easy to
//! support multiple ways of creating nodes.
//!
//! ## Features
//! The 3Delight dynamic library (`lib3delight`) can either be linked to
//! during build or loaded at runtime.
//! By default the lib is loaded at runtime. This has several
//! advantages:
//! 1. If you ship your application or library you can ship it without
//!    the library. It can still run and will print an informative error
//!    if the library cannot be loaded.
//! 2. A user can install an updated version of the renderer and stuff
//!    will ‘just work’.
//!
//! * Dynamically link against `lib3delight`.
//!   * `lib3delight` becomes a depdency. If it cannot't be found your
//!     lib/app will not load/start.
//!   * The feature is called `link_lib3delight`.
//! * Download `lib3delight` during build.
//!   * `lib3delight` is downloaded during build. Note that this may be
//!     an outdated version. This feature mainly exists for CI purposes.
//!   * The feature is called `download_lib3delight`.
#![allow(non_snake_case)]

#[cfg(not(feature = "link_lib3delight"))]
#[macro_use]
extern crate dlopen_derive;

use nsi_sys::*;

#[cfg(not(feature = "link_lib3delight"))]
mod dynamic;
#[cfg(feature = "link_lib3delight")]
mod linked;

#[cfg(not(feature = "link_lib3delight"))]
use self::dynamic as api;
#[cfg(feature = "link_lib3delight")]
use self::linked as api;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref NSI_API: api::ApiImpl = api::ApiImpl::new().expect("Could not load lib3delight.");
}

#[macro_use]
pub mod argument;
pub mod context;

mod tests;

pub use crate::argument::*;
pub use crate::context::*;

pub mod prelude {
    //! Re-exports commonly used types and traits.
    //!
    //! Importing the contents of this module is recommended.

    pub use crate::argument::Arg;
    pub use crate::argument::*;

    pub use crate::context::Context;
    pub use crate::context::NodeType;
    pub use crate::context::*;

    pub use crate::*;
}

trait Api {
    fn NSIBegin(&self, nparams: ::std::os::raw::c_int, params: *const NSIParam_t) -> NSIContext_t;
    fn NSIEnd(&self, ctx: NSIContext_t);
    fn NSICreate(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        type_: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDelete(
        &self,
        ctx: NSIContext_t,
        handle: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSISetAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSISetAttributeAtTime(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        time: f64,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDeleteAttribute(
        &self,
        ctx: NSIContext_t,
        object: NSIHandle_t,
        name: *const ::std::os::raw::c_char,
    );
    fn NSIConnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIDisconnect(
        &self,
        ctx: NSIContext_t,
        from: NSIHandle_t,
        from_attr: *const ::std::os::raw::c_char,
        to: NSIHandle_t,
        to_attr: *const ::std::os::raw::c_char,
    );
    fn NSIEvaluate(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
    fn NSIRenderControl(
        &self,
        ctx: NSIContext_t,
        nparams: ::std::os::raw::c_int,
        params: *const NSIParam_t,
    );
}
