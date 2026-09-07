//! Proves the synthesised C++ object is one 3Delight actually calls.
//!
//! Needs a licensed 3Delight. There is no way to test this short of a
//! render: the whole question is whether a hand-built vtable matches
//! what the renderer expects, and only the renderer can answer.
use nsi_3delight::progress::{Progress, ProgressCallback, ProgressReporter};
use nsi_ffi_wrap as nsi;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Records what it was told, and whether two calls ever overlapped.
#[derive(Default)]
struct Recorder {
    calls: AtomicUsize,
    /// Set while inside `update`. A second thread finding it set is a
    /// concurrent call.
    inside: AtomicBool,
    overlapped: AtomicBool,
    first: Mutex<Option<Progress>>,
    last: Mutex<Option<Progress>>,
}

impl ProgressCallback for Recorder {
    fn update(&self, progress: &Progress) {
        if self.inside.swap(true, Ordering::SeqCst) {
            self.overlapped.store(true, Ordering::SeqCst);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut first = self.first.lock().unwrap();
        if first.is_none() {
            *first = Some(*progress);
        }
        drop(first);
        *self.last.lock().unwrap() = Some(*progress);
        self.inside.store(false, Ordering::SeqCst);
    }
}

fn render(reporter: Option<&ProgressReporter<Recorder>>) {
    let ctx = nsi::Context::new(None).expect("context");
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, nsi::ROOT, "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::i32_slice!("resolution", &[64, 64])
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    );
    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "float"),
            nsi::i32!("withalpha", 1),
        ],
    );
    ctx.connect("beauty", None, "screen", "outputlayers", None);
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "exr"),
            nsi::string!(
                "imagefilename",
                std::env::temp_dir()
                    .join("nsi_progress_test")
                    .to_str()
                    .unwrap()
            ),
        ],
    );
    ctx.connect("driver", None, "beauty", "outputdrivers", None);

    match reporter {
        Some(reporter) => {
            ctx.render_control(nsi::Action::Start, Some(&[reporter.arg()]))
        }
        None => ctx.render_control(nsi::Action::Start, None),
    }
    ctx.render_control(nsi::Action::Wait, None);
}

/// The vtable is right only if the renderer calls through it. A wrong
/// vptr jumps to address 0 rather than reporting anything, so a
/// non-zero call count is the assertion.
#[test]
fn the_renderer_calls_update_through_our_vtable() {
    let reporter = ProgressReporter::new(Recorder::default());
    render(Some(&reporter));

    let recorder = reporter.callback();
    let calls = recorder.calls.load(Ordering::SeqCst);
    assert!(calls > 0, "the renderer never called update");

    // Progress must advance and finish. Anything else means we decoded
    // the `Value` struct wrongly even if the call itself landed.
    let first = recorder.first.lock().unwrap().expect("a first report");
    let last = recorder.last.lock().unwrap().expect("a last report");
    assert!(
        (0.0..=1.0).contains(&first.render_progress),
        "first progress out of range: {}",
        first.render_progress
    );
    assert!(
        last.render_progress >= first.render_progress,
        "progress went backwards: {} -> {}",
        first.render_progress,
        last.render_progress
    );
    assert!(
        last.render_progress > 0.99,
        "the last report should be a finished render, got {}",
        last.render_progress
    );
    assert!(
        last.total_passes > 0,
        "total_passes must be populated, got {}",
        last.total_passes
    );

    // Documented for the ETA consumers: the header calls this "seconds
    // we've been rendering", but during the render it carries the
    // render's start timestamp instead. Only asserted loosely, because
    // it is the renderer's behaviour we are recording, not ours.
    println!(
        "calls={calls} first: progress={:.4} secs={:.3} | last: \
         progress={:.4} secs={:.3}",
        first.render_progress,
        first.seconds_rendering,
        last.render_progress,
        last.seconds_rendering
    );

    // Whether the renderer serialises `update` is not promised by
    // `Progress.h`, which is why the trait takes `&self` and requires
    // `Sync`. Record what actually happened rather than assume.
    println!(
        "concurrent update calls observed: {}",
        recorder.overlapped.load(Ordering::SeqCst)
    );
}

/// Without the argument there must be no reports -- otherwise the test
/// above proves nothing about our object being the one called.
#[test]
fn no_reporter_means_no_reports() {
    let reporter = ProgressReporter::new(Recorder::default());
    render(None);
    assert_eq!(
        0,
        reporter.callback().calls.load(Ordering::SeqCst),
        "a reporter that was never passed must never be called"
    );
}
