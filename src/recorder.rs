//! The ɴsɪ recorder.
//!
//! `Nsi` takes `&self` everywhere, so the scene lives behind a `Mutex`.

use crate::{ClassifyError, OwnedArg, Scene};
use nsi_ffi_wrap::Arg;
use nsi_trait::{Action, Nsi};
use std::sync::{Mutex, MutexGuard};

/// Where the render is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderState {
    #[default]
    Idle,
    Running,
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
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the recorded scene.
    pub fn scene(&self) -> MutexGuard<'_, Scene> {
        self.scene.lock().expect("scene mutex poisoned")
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

    type Error = ClassifyError;

    fn create(
        &self,
        handle: &str,
        node_type: &str,
        _args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.scene().create(handle, node_type);
        Ok(())
    }

    fn delete(&self, handle: &str, _args: Option<&[Self::Arg<'_>]>) -> Result<(), Self::Error> {
        self.scene().delete(handle);
        Ok(())
    }

    fn set_attribute(&self, handle: &str, args: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        let owned = Self::own(args);
        self.scene().set_attribute(handle, owned);
        Ok(())
    }

    fn set_attribute_at_time(
        &self,
        handle: &str,
        time: f64,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        let owned = Self::own(args);
        self.scene().set_attribute_at_time(handle, time, owned);
        Ok(())
    }

    fn delete_attribute(&self, handle: &str, name: &str) -> Result<(), Self::Error> {
        self.scene().delete_attribute(handle, name);
        Ok(())
    }

    fn connect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        _args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.scene().connect(from, from_attr, to, to_attr)
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

    fn evaluate(&self, _args: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        // Procedurals and Lua are out of scope until a backend exists.
        // Recording them would imply an execution model we have not
        // designed, and a silent no-op is easier to reason about than a
        // half-recorded one.
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
mod tests {
    use super::*;
    use nsi_ffi_wrap as nsi;
    use nsi_trait::{Action, Nsi};

    fn assert_is_nsi<T: Nsi>() {}

    #[test]
    fn recorder_implements_nsi() {
        assert_is_nsi::<Recorder>();
    }

    #[test]
    fn records_a_node_and_its_attribute() {
        let r = Recorder::new();
        r.create("cam", "perspectivecamera", None).unwrap();
        r.set_attribute("cam", &[nsi::f32!("fov", 45.0)]).unwrap();

        let scene = r.scene();
        assert_eq!(scene.nodes["cam"].node_type, "perspectivecamera");
        assert_eq!(scene.nodes["cam"].attrs["fov"].name, "fov");
    }

    #[test]
    fn an_unmapped_connection_is_an_error() {
        let r = Recorder::new();
        r.create("a", "transform", None).unwrap();
        r.create("b", "transform", None).unwrap();
        let err = r.connect("a", None, "b", "nonsense", None).unwrap_err();
        assert_eq!(err.to_attr, "nonsense");
    }

    #[test]
    fn render_control_drives_the_state_machine() {
        let r = Recorder::new();
        assert_eq!(r.render_state(), RenderState::Idle);
        r.render_control(Action::Start, None).unwrap();
        assert_eq!(r.render_state(), RenderState::Running);
        r.render_control(Action::Suspend, None).unwrap();
        assert_eq!(r.render_state(), RenderState::Suspended);
        r.render_control(Action::Resume, None).unwrap();
        assert_eq!(r.render_state(), RenderState::Running);
        r.render_control(Action::Stop, None).unwrap();
        assert_eq!(r.render_state(), RenderState::Idle);
    }

    /// `Wait` and `Synchronize` are not state transitions. A recorder
    /// has nothing to wait for, and synchronising an unrendered scene is
    /// a no-op.
    #[test]
    fn wait_and_synchronize_do_not_change_state() {
        let r = Recorder::new();
        r.render_control(Action::Start, None).unwrap();
        r.render_control(Action::Synchronize, None).unwrap();
        assert_eq!(r.render_state(), RenderState::Running);
        r.render_control(Action::Wait, None).unwrap();
        assert_eq!(r.render_state(), RenderState::Running);
    }

    /// The trait demands `Send + Sync`, and the scene transitively holds
    /// raw host pointers. This compiles only if that is handled.
    #[test]
    fn recorder_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Recorder>();
    }
}
