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
//! # The ɴsɪ copy contract
//!
//! Every ɴsɪ argument except a `NSIType` pointer (`Type::Reference`) is
//! copied during the call, so a caller may free its data as soon as the
//! call returns. The recorder copies for the same reason a renderer
//! does; it introduces no cost that a live context would not have paid.
//! `Reference` is the exception: it is passed through, retained, and is
//! why the recorder carries a context lifetime.

/// ɴsɪ's root node handle.
pub const ROOT: &str = ".root";

mod edge;
mod owned;
mod recorder;
mod resolve;
mod scene;
mod stream;

pub use edge::{ClassifyError, Edge, EdgeKind, classify};
pub use owned::{HostPtr, OwnedArg, OwnedData};
pub use recorder::{Recorder, RenderState};
pub use resolve::{Binding, IDENTITY, OutputLayer, RenderOutput};
pub use scene::{Node, Scene};
pub use stream::write_stream;
