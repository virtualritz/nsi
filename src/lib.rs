//! A renderer-agnostic recorder for the Nodal Scene Interface.
//!
//! This crate records an ɴsɪ scene — nodes, attributes and classified
//! connections — and replays it. Recording is pure Rust and needs no
//! renderer present, and nothing here is specific to any one renderer.
//!
//! Backends consume a recorded scene and flush it into their own
//! representation: `nsi-mitsuba` into Mitsuba `Properties`, a future
//! `nsi-moonray` into `scene_rdl2` `SceneObject`s.
//!
//! # The ɴsɪ copy contract
//!
//! Every ɴsɪ argument except a `NSIType` pointer (`Type::Reference`) is
//! copied during the call, so a caller may free its data as soon as the
//! call returns. The recorder copies for the same reason a renderer
//! does; it introduces no cost that a live context would not have paid.
//! `Reference` is the exception: it is passed through, retained, and is
//! why the recorder carries a context lifetime.

mod owned;

pub use owned::{OwnedArg, OwnedData};
