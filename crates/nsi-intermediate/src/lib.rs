//! A renderer-agnostic intermediate representation for the Nodal Scene
//! Interface.
//!
//! ɴsɪ is the front end and a renderer is the back end; this is what
//! sits between them. It does the jobs an IR does — capture the
//! incoming calls, classify and canonicalise them, lower ɴsɪ's graph
//! semantics into flat facts a renderer can consume, and serialise the
//! result for inspection.
//!
//! Nothing here is specific to any one renderer, and none of it needs a
//! renderer present to build or test.
//!
//! Backends consume a lowered scene and flush it into their own
//! representation: [`nsi-mitsuba`] into Mitsuba `Properties`,
//! [`nsi-moonray`] into `scene_rdl2` `SceneObject`s.
//!
//! # Naming
//!
//! The crate is spelled out in full so the name says what it is.
//! Consumers who prefer the shorter idiom may alias it:
//!
//! ```ignore
//! use nsi_intermediate as nsi_ir;
//! ```
//!
//! [`nsi-mitsuba`]: https://github.com/virtualritz/nsi-mitsuba
//! [`nsi-moonray`]: https://github.com/virtualritz/nsi-moonray
//!
//! # Writing a backend
//!
//! Record with [`Recorder`], which implements [`nsi_trait::Nsi`], then
//! ask the [`Scene`] for facts rather than walking it yourself:
//!
//! ```ignore
//! let recorder = Recorder::new();
//! build_the_scene(&recorder)?;          // any generic ɴsɪ consumer.
//! let scene = recorder.scene();
//!
//! for output in scene.render_outputs() {
//!     // one entry per screen, with its layers and drivers in order.
//! }
//!
//! for (handle, node) in &scene.nodes {
//!     if node.node_type != "mesh" {
//!         continue;
//!     }
//!
//!     // Material, gathered along the whole path to `.root`.
//!     if let Some(binding) = scene.geometry_binding(handle)? {
//!         let shader = binding.surface_shader;
//!         // `binding.attributes` is every `attributes` node on the
//!         // path, in ɴsɪ's precedence order: take the first that
//!         // defines the attribute you want.
//!     }
//!
//!     // Transform. Ask whether it moves before asking where it is.
//!     let matrices = if scene.motion_times(handle)?.is_empty() {
//!         vec![(0.0, scene.world_transform(handle)?)]
//!     } else {
//!         scene.world_transform_samples(handle)?
//!     };
//! }
//! ```
//!
//! # What it refuses to answer
//!
//! ɴsɪ permits scenes with no single correct answer, and this crate
//! returns a typed error for each rather than a plausible wrong one: a
//! node with two `objects` parents (ɴsɪ's lightweight instancing), a
//! cycle, a node that never reaches `.root`, and a motion-sampled
//! transform asked for at a time it has no sample at. Nothing here
//! interpolates between motion samples -- element-wise interpolation of
//! a matrix is wrong for anything containing a rotation, and the right
//! decomposition is the backend's to choose.
//!
//! # What it does not resolve
//!
//! Shader networks are classified and carried with their ports intact,
//! because their consumer is OSL rather than a graph walk. `evaluate`
//! is a no-op: procedurals and Lua imply an execution model this
//! surface does not define. An instanced node is detected and refused,
//! not expanded into one transform per path.
//!
//! # The ɴsɪ copy contract
//!
//! Every ɴsɪ argument except a `NSIType` pointer (`Type::Reference`) is
//! copied during the call, so a caller may free its data as soon as the
//! call returns. The recorder copies for the same reason a renderer
//! does; it introduces no cost that a live context would not have paid.
//! `Reference` is the exception: it is passed through, retained, and is
//! why the recorder carries a context lifetime.

#![deny(missing_docs)]

/// ɴsɪ's root node handle.
pub const ROOT: &str = ".root";

/// ɴsɪ's global-options node handle.
///
/// Reserved like [`ROOT`]: it need not be created.
pub const GLOBAL: &str = ".global";

/// ɴsɪ's wildcard handle and attribute name.
///
/// `NSIDisconnect` accepts it for either node handle and for either
/// attribute name, matching everything in that position.
pub const ALL: &str = ".all";

mod edge;
mod error;
mod owned;
mod recorder;
mod resolve;
mod scene;
mod stream;

pub use edge::{ClassifyError, Edge, EdgeKind, classify};
pub use error::RecordError;
pub use owned::{HostPtr, OwnedArg, OwnedData};
pub use recorder::{Recorder, RenderState};
pub use resolve::{Binding, IDENTITY, OutputLayer, RenderOutput, ResolveError};
pub use scene::{Node, Scene};
pub use stream::write_stream;
