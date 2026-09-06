//! Tests for [`super`].
//!
//! Separate file per the workspace rule: source files do not grow
//! inline `#[cfg(test)]` modules.

use super::*;
use crate::{EdgeKind, OwnedData};
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

/// ɴsɪ's destinations are an open set -- its own §4.8 connects a node to
/// another's `visibility` -- so an unlisted one is carried rather than
/// refused. It is never *resolved*, which is what keeps the failure this
/// classifier exists to prevent: a connection is only interpreted as a
/// material or an output route when its name says so.
#[test]
fn an_unlisted_connection_is_carried_not_interpreted() {
    let r = Recorder::new();
    r.create("a", "transform", None).unwrap();
    r.create("b", "transform", None).unwrap();
    r.connect("a", None, "b", "nonsense", None).unwrap();

    let scene = r.scene();
    assert_eq!(
        scene.edges().next().expect("carried").kind,
        EdgeKind::Other {
            to_attr: "nonsense".to_string()
        }
    );
    // ...and it never becomes a material. `b` is not in the scene, so
    // ask about the node that is.
    drop(scene);
    r.connect("b", None, ".root", "objects", None).unwrap();
    assert!(r.scene().geometry_binding("b").unwrap().is_none());
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
fn an_unlisted_disconnect_removes_the_edge_it_names() {
    let r = Recorder::new();
    r.create("a", "transform", None).unwrap();
    r.create("b", "transform", None).unwrap();
    r.connect("a", None, "b", "nonsense", None).unwrap();
    r.disconnect("a", None, "b", "nonsense").unwrap();
    assert_eq!(r.scene().edges().count(), 0);
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

/// `create`'s arguments are dropped, and that loses nothing.
///
/// ɴsɪ: "nparams, params ... There are no optional parameters defined
/// as of now." The specification also says a repeated `create` "does
/// nothing if all other parameters match the call which created that
/// node. Otherwise, it emits an error", which reads as though the
/// arguments were part of a node's identity.
///
/// Rendered, they are not. 3Delight accepts
/// `Create "n" "attributes" "foo" "int" 1 [1]` followed by the same
/// with `[2]` without complaint, and the node works. It *does* refuse a
/// repeat with a different **type**: `E6002 error creating node 'extra'
/// of type 'transform', already exists as type 'attributes'`. So the
/// type is the identity and the arguments are inert.
#[test]
fn create_arguments_are_inert_but_the_type_is_not() {
    let recorder = Recorder::new();
    recorder
        .create("n", "attributes", Some(&[nsi::i32!("foo", 1)]))
        .expect("first create");
    recorder
        .create("n", "attributes", Some(&[nsi::i32!("foo", 2)]))
        .expect("3Delight accepts a differing create argument");

    assert!(
        matches!(
            recorder.create("n", "transform", None),
            Err(RecordError::TypeMismatch { .. })
        ),
        "a differing type is E6002 in 3Delight",
    );

    let scene = recorder.into_scene();
    assert_eq!(scene.node("n").unwrap().node_type, "attributes");
    assert!(
        scene.node("n").unwrap().attrs.is_empty(),
        "a create argument is not an attribute",
    );
}
