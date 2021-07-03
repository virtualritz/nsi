//#![warn(missing_docs)]
//#![warn(missing_doc_code_examples)]
//! # Nodal Scene Interface – ɴsɪ
//! A flexible, modern API for offline 3D renderers
//!
//! [Nsɪ](https://nsi.readthedocs.io/) is built around the concept of
//! nodes. Each node has a *unique handle* to identify it. It also has
//! a [type](nsi_core::context::NodeType) which describes its intended function
//! in the scene.
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
//!     These are attached to a rendering
//! [`Context`](nsi_core::context::Context).
//!
//! 2.  Nodes of different [`NodeType`](nsi_core::context::NodeType)s understood
//! by the renderer.
//!
//! Much of the complexity and expressiveness of the interface comes
//! from
//! [the supported nodes](https://nsi.readthedocs.io/en/latest/nodes.html).
//!
//! The first part was kept deliberately simple to make it easy to
//! support multiple ways of creating nodes.
//!
//! ## Example
//!
//! ```
//! // Create a context to send the scene to.
//! let ctx = nsi::Context::new(None).expect("Could not create NSI context.");
//!
//! // Create a dodecahedron.
//! let face_index: [i32; 60] =
//!     // 12 regular pentagon faces.
//!    [
//!          0, 16,  2, 10,  8,  0,  8,  4, 14, 12,
//!         16, 17,  1, 12,  0,  1,  9, 11,  3, 17,
//!          1, 12, 14,  5,  9,  2, 13, 15,  6, 10,
//!         13,  3, 17, 16,  2,  3, 11,  7, 15, 13,
//!          4,  8, 10,  6, 18, 14,  5, 19, 18,  4,
//!          5, 19,  7, 11,  9, 15,  7, 19, 18,  6,
//!     ];
//! let positions: [f32; 60] =
//!     // 20 points @ 3 vertices.
//!     [
//!          1.,     1.,     1.,     1.,     1.,    -1.,
//!          1.,    -1.,     1.,     1.,    -1.,    -1.,
//!         -1.,     1.,     1.,    -1.,     1.,    -1.,
//!         -1.,    -1.,     1.,    -1.,    -1.,    -1.,
//!          0.,     0.618,  1.618,  0.,     0.618, -1.618,
//!          0.,    -0.618,  1.618,  0.,    -0.618, -1.618,
//!          0.618,  1.618,  0.,     0.618, -1.618,  0.,
//!        -0.618,  1.618,  0.,    -0.618, -1.618,  0.,
//!          1.618,  0.,     0.618,  1.618,  0.,    -0.618,
//!         -1.618,  0.,     0.618, -1.618,  0.,    -0.618,
//!     ];
//!
//! // Create a new mesh node and call it 'dodecahedron'.
//! ctx.create("dodecahedron", nsi::NodeType::Mesh, None);
//!
//! // Connect the 'dodecahedron' node to the scene's root.
//! ctx.connect("dodecahedron", "", ".root", "objects", None);
//!
//! // Define the geometry of the 'dodecahedron' node.
//! ctx.set_attribute(
//!     "dodecahedron",
//!     &[
//!         nsi::points!("P", &positions),
//!         nsi::integers!("P.indices", &face_index),
//!         // 5 vertices per each face.
//!         nsi::integers!("nvertices", &[5; 12]),
//!         // Render this as a subdivison surface.
//!         nsi::string!("subdivision.scheme", "catmull-clark"),
//!         // Crease each of our 30 edges a bit.
//!         nsi::integers!("subdivision.creasevertices", &face_index),
//!         nsi::floats!("subdivision.creasesharpness", &[10.; 30]),
//!     ],
//! );
//! ```
//! ## More Examples
//! All the examples in this crate require a (free) [3Delight](https://www.3delight.com/)
//! installation to run.
//!
//! ### Interactive
//! Demonstrates using the [`FnStatus`](nsi_core::context::FnStatus) callback
//! closure during rendering.
//!
//! ### Jupyter
//! Render directly into a Jupyter notebook.
//!
//! Follow
//! [these instructions](https://github.com/google/evcxr/blob/master/evcxr_jupyter/README.md) to
//! get a Rust Jupyter kernel up and running first.
//!
//! ### Output
//! This is a full [`output`](nsi_core::output) example showing color conversion
//! and writing data out to 8bit/channel PNG and 32bit/channel (float) OpenEXR
//! formats.
//!
//! ### Volume
//! Demonstrates rendering an [OpenVDB](https://www.openvdb.org/) asset. Mostly through the
//! [`toolbelt`](crate::toolbelt) helpers.
//!
//! ## Getting Pixels
//! The crate has support for streaming pixels from the renderer, via callbacks
//! (i.e. closures) during and/or after rendering via the
//! [`output`](nsi_core::output) module. This module is enabled through the
//! feature of the same name (see below).
//!
//! ## Cargo Features
//! * [`output`](nsi_core::output) – Add support for streaming pixels from the
//!   renderer to the calling context via closures.
//!
//! * [`jupyter`](crate::jupyter) – Add support for rendering to Jupyter notebooks (when using a [Rust Jupyter
//!    kernel](https://github.com/google/evcxr)).
//!
//! * [`toolbelt`](crate::toolbelt) – Add convenience methods that work with a
//!   [`Context`](nsi_core::context::Context).
//!
//! * `nightly` – Enable some unstable features (suggested if you build with a
//!   `nightly` toolchain)
//!
//! ## Linking Style
//! The 3Delight dynamic library (`lib3delight`) can either be linked to during
//! build or loaded at runtime.
//!
//! By default the lib is loaded at runtime.
//!
//! * Load `lib3deligh` at runtime (default). This has several advantages: 1. If
//!   you ship your application or library you can ship it without the library.
//!   It can still run and will print an informative error if the library cannot
//!   be loaded.
//!
//!   2. A user can install an updated version of the renderer and stuff will
//! ‘just work’.
//! * Dynamically link against `lib3delight`.
//!   * `lib3delight` becomes a depdency. If it cannot't be found your lib/app
//!     will not load/start.
//!
//!   * The feature is called `link_lib3delight`.
//!
//!   * You should disable default features (they are not needed/used) in this
//!     case:
//!
//!     ```toml
//!     [dependencies.nsi]
//!     version = "0.5.5"
//!     features = [ "link_lib3delight" ]
//!     default-features = false
//!     ```
//!
//! * Download `lib3delight` during build.
//!   * `lib3delight` is downloaded during build. Note that this may be an
//!     outdated version. This feature mainly exists for CI purposes.
//!
//!   * The feature is called `download_lib3delight`.

pub use nsi_core::*;
pub use nsi_internal::*;
