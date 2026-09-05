//! The ɴsɪ recorder.
//!
//! `Nsi` takes `&self` everywhere, so the scene lives behind a `Mutex`.

use crate::{OwnedArg, RecordError, Scene};
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

    fn delete(
        &self,
        handle: &str,
        _args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        self.scene().delete(handle)
    }

    fn set_attribute(
        &self,
        handle: &str,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
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
    use crate::{ClassifyError, OwnedData};
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
        assert_eq!(scene.node("cam").unwrap().node_type, "perspectivecamera");
        assert_eq!(scene.node("cam").unwrap().attrs["fov"].name, "fov");
    }

    #[test]
    fn an_unmapped_connection_is_an_error() {
        let r = Recorder::new();
        r.create("a", "transform", None).unwrap();
        r.create("b", "transform", None).unwrap();
        let err = r.connect("a", None, "b", "nonsense", None).unwrap_err();
        assert_eq!(
            err,
            RecordError::Classify(ClassifyError {
                to_attr: "nonsense".to_string()
            })
        );
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

    /// `Nsi::delete`, not `Scene::delete`. The trait method is the only
    /// one a consumer can reach, and nothing else drives it.
    #[test]
    fn delete_through_the_trait_removes_the_node_and_its_edges() {
        let r = Recorder::new();
        r.create("xf", "transform", None).unwrap();
        r.create("mesh", "mesh", None).unwrap();
        r.connect("mesh", None, "xf", "objects", None).unwrap();
        assert_eq!(r.scene().edges().count(), 1);

        r.delete("xf", None).unwrap();

        let scene = r.scene();
        assert!(!scene.node("xf").is_some());
        assert!(scene.edges().next().is_none());
    }

    /// `Nsi::disconnect` removes the edge it names and leaves the rest.
    #[test]
    fn disconnect_through_the_trait_removes_one_edge() {
        let r = Recorder::new();
        r.create("a", "transform", None).unwrap();
        r.create("b", "transform", None).unwrap();
        r.connect("a", None, ".root", "objects", None).unwrap();
        r.connect("b", None, ".root", "objects", None).unwrap();

        r.disconnect("a", None, ".root", "objects").unwrap();

        let scene = r.scene();
        assert_eq!(scene.edges().count(), 1);
        assert_eq!(scene.edges().next().unwrap().from, "b");
    }

    /// An unmapped destination must fail on the way out as well as in.
    /// Classifying is how `disconnect` finds the edge to remove, so a
    /// destination it cannot classify cannot be a silent no-op.
    #[test]
    fn an_unmapped_disconnect_is_an_error() {
        let r = Recorder::new();
        let err = r.disconnect("a", None, "b", "nonsense").unwrap_err();
        assert_eq!(
            err,
            RecordError::Classify(ClassifyError {
                to_attr: "nonsense".to_string()
            })
        );
    }

    /// `evaluate` is a no-op by decision, not omission: procedurals and
    /// Lua imply an execution model this crate does not define. See the
    /// `spec.md` non-goal.
    #[test]
    fn evaluate_is_a_recorded_no_op() {
        let r = Recorder::new();
        r.create("cam", "perspectivecamera", None).unwrap();
        let before = r.scene().clone();

        r.evaluate(&[nsi::string!("filename", "proc.lua")]).unwrap();

        assert_eq!(*r.scene(), before, "evaluate changed the scene");
    }

    /// ɴsɪ's `"priority"` is the one `connect` argument that survives,
    /// because `geometry_binding` needs it to choose between an
    /// inherited binding and a direct one.
    #[test]
    fn connect_records_the_priority_argument() {
        let r = Recorder::new();
        r.create("attr", "attributes", None).unwrap();
        r.create("mesh", "mesh", None).unwrap();
        r.connect(
            "attr",
            None,
            "mesh",
            "geometryattributes",
            Some(&[nsi::i32!("priority", 7)]),
        )
        .unwrap();

        assert_eq!(r.scene().edges().next().unwrap().priority(), 7);
        assert_eq!(r.scene().len(), 2);
    }

    /// A `Reference` driven through the trait, which is the only path a
    /// consumer has and the only one where the `'static` pin on the
    /// `Arg` GAT applies. `owned::tests` calls `from_param` directly and
    /// so proves the marshalling but not this.
    #[test]
    fn a_reference_through_the_trait_records_the_host_address() {
        static PAYLOAD: u64 = 0xdead_beef_cafe_f00d;
        let expected = &raw const PAYLOAD as usize;

        let r = Recorder::new();
        r.create("driver", "outputdriver", None).unwrap();
        r.set_attribute(
            "driver",
            &[nsi::reference_stable!("callbackdata", &PAYLOAD)],
        )
        .unwrap();

        let scene = r.scene();
        match &scene.node("driver").unwrap().attrs["callbackdata"].data {
            OwnedData::Reference(pointers) => {
                assert_eq!(pointers.len(), 1);
                assert_eq!(pointers[0].0 as usize, expected);
            }
            other => panic!("expected Reference, got {other:?}"),
        }
    }

    /// A `Callback` argument leaks its payload, and this pins that as a
    /// known limitation rather than letting it stay invisible.
    ///
    /// `Callback::type_` reports `Type::Reference`, so the recorder
    /// cannot tell one from a plain `Reference`, and `Callback::drop_fn`
    /// is `pub(crate)` to `nsi-ffi-wrap`, so it could not call it even
    /// if it could. A `Context` takes ownership and frees it; a
    /// `Recorder` records the address and nothing more. See
    /// `contracts/recording.md`.
    #[test]
    fn a_callback_records_its_address_and_leaks_its_payload() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static RECLAIMED: AtomicUsize = AtomicUsize::new(0);

        struct Payload(u64);

        impl nsi::CallbackPtr for Payload {
            fn to_ptr(self) -> *const std::ffi::c_void {
                Box::into_raw(Box::new(self)).cast()
            }

            unsafe fn drop_ptr(ptr: *const std::ffi::c_void) {
                let payload = unsafe { Box::from_raw(ptr as *mut Payload) };
                RECLAIMED.fetch_add(payload.0 as usize, Ordering::SeqCst);
            }
        }

        {
            let r = Recorder::new();
            r.create("driver", "outputdriver", None).unwrap();
            r.set_attribute("driver", &[nsi::callback!("cb", Payload(1))])
                .unwrap();

            match &r.scene().node("driver").unwrap().attrs["cb"].data {
                OwnedData::Reference(pointers) => {
                    assert_eq!(pointers.len(), 1);
                    assert!(!pointers[0].0.is_null(), "address recorded");
                }
                other => panic!("expected Reference, got {other:?}"),
            }
        }

        assert_eq!(
            RECLAIMED.load(Ordering::SeqCst),
            0,
            "a Recorder cannot reclaim a Callback payload; if this ever \
             becomes 1, the leak is fixed and the contract row must change"
        );
    }
}
