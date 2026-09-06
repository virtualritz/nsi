//! The ɴsɪ recorder.
//!
//! `Nsi` takes `&self` everywhere, so the scene lives behind a `Mutex`.

use crate::{OwnedArg, OwnedData, RecordError, Scene};
use nsi_ffi_wrap::Arg;
use nsi_trait::{Action, Nsi};
use std::sync::{Mutex, MutexGuard};

/// Where the render is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RenderState {
    /// Not rendering. The state before `Start` and after `Stop`.
    #[default]
    Idle,
    /// Rendering.
    Running,
    /// Started, then suspended. `Resume` returns it to
    /// [`RenderState::Running`].
    Suspended,
}

/// Records an ɴsɪ scene without rendering it.
///
/// # Why there is no context lifetime
///
/// [`nsi_ffi_wrap::Context`] carries a `'a` bounding the borrowed data
/// handed to it through `Reference`, `ReferenceSlice` and `Callback`.
/// The recorder does not, and the difference is real rather than an
/// oversight: `Context` stores no pointers — its `'a` is a `PhantomData`
/// marker and the renderer holds the data — whereas the recorder
/// *retains* those addresses so they survive to replay.
///
/// Retaining them while also being `Send + Sync`, which [`Nsi`]
/// requires, is only sound if the pointees outlive every thread that
/// could see them. So the `Arg` GAT is pinned to `'static` and a
/// lifetime parameter would be vestigial: it could only ever be
/// `'static`. This matches `nsi-ffi-wrap`, where `Reference`,
/// `Callback` and `ReferenceSlice` are `Send`/`Sync` at `'static` and
/// nowhere else.
///
/// In practice this costs nothing. ɴsɪ `Reference` carries output-driver
/// callbacks and similar host state, which is long-lived by nature.
#[derive(Debug, Default)]
pub struct Recorder {
    scene: Mutex<Scene>,
    state: Mutex<RenderState>,
}

impl Recorder {
    /// An empty recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the recorded scene.
    ///
    /// # Deadlock
    ///
    /// The guard holds the only lock every [`Nsi`] method takes. Calling
    /// one on the same recorder while a guard is alive deadlocks the
    /// calling thread. Drop the guard, or scope it, before recording
    /// again.
    pub fn scene(&self) -> MutexGuard<'_, Scene> {
        self.scene.lock().expect("scene mutex poisoned")
    }

    /// Take the recorded scene, consuming the recorder.
    ///
    /// The alternative is `scene().clone()`, which deep-copies every
    /// vertex buffer in the scene. A backend that is done recording
    /// wants this one.
    ///
    /// # Panics
    ///
    /// If the scene mutex was poisoned by a panic while recording.
    pub fn into_scene(self) -> Scene {
        self.scene.into_inner().expect("scene mutex poisoned")
    }

    /// The current render state.
    pub fn render_state(&self) -> RenderState {
        *self.state.lock().expect("state mutex poisoned")
    }

    fn own(args: &[Arg<'_, 'static>]) -> Vec<OwnedArg> {
        args.iter().map(OwnedArg::from_param).collect()
    }
}

impl Nsi for Recorder {
    /// `'call` is the transient borrow. The context-bound lifetime is
    /// `'static`; see the type's documentation for why.
    type Arg<'call> = Arg<'call, 'static>;
    type Error = RecordError;

    fn create(
        &self,
        handle: &str,
        node_type: &str,
        _args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.scene().create(handle, node_type)
    }

    /// `args` is read for ɴsɪ's `"recursive"`.
    fn delete(
        &self,
        handle: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        let recursive = args
            .unwrap_or_default()
            .iter()
            .map(OwnedArg::from_param)
            .find(|arg| arg.name == "recursive")
            .is_some_and(|arg| match &arg.data {
                OwnedData::I32(values) => {
                    values.first().is_some_and(|v| *v != 0)
                }
                _ => false,
            });

        if recursive {
            self.scene().delete_recursive(handle)
        } else {
            self.scene().delete(handle)
        }
    }

    fn set_attribute(
        &self,
        handle: &str,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        let owned = Self::own(args);
        self.scene().set_attribute(handle, owned)
    }

    fn set_attribute_at_time(
        &self,
        handle: &str,
        time: f64,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        let owned = Self::own(args);
        self.scene().set_attribute_at_time(handle, time, owned)
    }

    fn delete_attribute(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<(), Self::Error> {
        self.scene().delete_attribute(handle, name);
        Ok(())
    }

    /// Every connection argument is recorded. Resolution reads only
    /// `"priority"`, but `"strength"` and `"value"` survive for a
    /// backend that wants them, and for replay.
    fn connect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        let args = args.map(Self::own).unwrap_or_default();
        self.scene()
            .connect_with_args(from, from_attr, to, to_attr, args)
    }

    fn disconnect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), Self::Error> {
        self.scene().disconnect(from, from_attr, to, to_attr)
    }

    fn evaluate(&self, args: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        // Not executed -- an archive, Lua script or compiled procedural
        // implies an execution model this crate does not define -- but
        // recorded. Dropping it meant a stream carrying `Evaluate` came
        // back as a scene missing whatever it would have produced, with
        // no error and nothing to show that anything had been asked
        // for. `Scene::evaluations` hands the call to a backend that
        // wants to run it.
        self.scene().evaluate(Self::own(args));
        Ok(())
    }

    fn render_control(
        &self,
        action: Action,
        _args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        let mut state = self.state.lock().expect("state mutex poisoned");
        *state = match (action, *state) {
            (Action::Start, _) => RenderState::Running,
            (Action::Suspend, RenderState::Running) => RenderState::Suspended,
            (Action::Resume, RenderState::Suspended) => RenderState::Running,
            (Action::Stop, _) => RenderState::Idle,
            // Wait and Synchronize are not transitions: a recorder has
            // nothing to wait for, and synchronising an unrendered scene
            // is a no-op.
            (_, current) => current,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests;
