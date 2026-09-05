//! Reproduction harness for concurrent interactive renders.
//!
//! Spins up `N` independent [`nsi::Context`]s, each driving its own
//! interactive + progressive render of a trivial (camera + environment dome)
//! scene through a FERRIS f32 output driver writing into a per-context buffer.
//!
//! Each context is driven start-to-teardown on its **own dedicated thread**
//! (matching akatela's working consumer pattern): the thread issues
//! `Start` + `Synchronize`, blocks until told to stop, then runs `Stop` +
//! `Wait`. The main thread:
//!   1. waits (with a timeout) for the *first* bucket on every context, and
//!   2. signals teardown and waits (with a timeout) for each thread to finish,
//!      so a `Stop`/`Wait` deadlock is *reported* instead of hanging.
//!
//! Configurable via env vars so we can A/B the suspected factors:
//!   * `NSI_CTX_COUNT`      (default 2)   — number of concurrent contexts.
//!   * `NSI_INTERACTIVE`    (default 1)   — 1 = interactive+progressive, 0 = batch.
//!   * `NSI_UNIQUE_NAMES`   (default 0)   — 1 = unique `imagefilename` per ctx.
//!   * `NSI_SYNC`           (default 1)   — issue a `Synchronize` after `Start`.
//!   * `NSI_STATUS`         (default 1)   — pass a status `callback` to `Start`.
//!   * `NSI_DRIVE`          (default thread) — `thread` = per-ctx thread,
//!     `main` = drive everything on main.
//!   * `NSI_RENDERTHREADS`  (unset)       — set `.global` `renderthreads` if given.
//!   * `NSI_FIRST_BUCKET_TIMEOUT` (default 20) — seconds to wait for buckets.
//!   * `NSI_TEARDOWN_TIMEOUT`     (default 8)  — seconds before declaring a
//!     teardown deadlock.

use nsi_ffi_wrap as nsi;
use std::{
    env,
    io::Write,
    num::NonZeroUsize,
    process::ExitCode,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Append a line to the NSI_LOG file (if set), flushing immediately, and also
/// print it. The file path survives 3Delight crash-spew that can otherwise
/// clobber the captured stdout of a concurrent-interactive run.
fn logln(msg: &str) {
    println!("{msg}");
    static LOG: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    let path = LOG.get_or_init(|| env::var_os("NSI_LOG").map(Into::into));
    if let Some(path) = path
        && let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(f, "{msg}");
        let _ = f.flush();
    }
}

/// Per-context diagnostics shared with the write/open callbacks.
struct Probe {
    label: String,
    opened: AtomicBool,
    buckets: AtomicU64,
    /// Milliseconds from program start to the first bucket; `u64::MAX` = none.
    first_bucket_ms: AtomicU64,
}

impl Probe {
    fn new(label: String) -> Arc<Self> {
        Arc::new(Self {
            label,
            opened: AtomicBool::new(false),
            buckets: AtomicU64::new(0),
            first_bucket_ms: AtomicU64::new(u64::MAX),
        })
    }
}

#[derive(Clone, Copy)]
struct Config {
    interactive: bool,
    unique_names: bool,
    do_sync: bool,
    use_status_callback: bool,
    renderthreads: Option<i32>,
    driver_name: &'static str,
    denoise: bool,
}

/// Dump this process's thread states (from /proc/self) — what each thread is
/// blocked on (`wchan`) and its scheduler state. 3Delight's render threads live
/// in *this* process, so this reveals where an interactive render is wedged
/// without needing ptrace/gdb.
fn dump_self_threads(tag: &str) {
    println!("\n=== /proc/self threads [{tag}] ===");
    let mut sockets = 0usize;
    if let Ok(fds) = std::fs::read_dir("/proc/self/fd") {
        for fd in fds.flatten() {
            if let Ok(target) = std::fs::read_link(fd.path())
                && target.to_string_lossy().contains("socket")
            {
                sockets += 1;
            }
        }
    }
    println!("open sockets: {sockets}");
    if let Ok(tasks) = std::fs::read_dir("/proc/self/task") {
        let mut lines: Vec<String> = Vec::new();
        for t in tasks.flatten() {
            let tid = t.file_name().to_string_lossy().into_owned();
            let comm = std::fs::read_to_string(t.path().join("comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let wchan = std::fs::read_to_string(t.path().join("wchan"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let state = std::fs::read_to_string(t.path().join("stat"))
                .ok()
                .and_then(|s| {
                    s.rsplit(')').next().and_then(|rest| {
                        rest.trim().split(' ').next().map(|s| s.to_string())
                    })
                })
                .unwrap_or_default();
            lines.push(format!(
                "tid={tid:>8} state={state} comm={comm:<16} wchan={wchan}"
            ));
        }
        lines.sort();
        for l in &lines {
            println!("{l}");
        }
        println!("(total threads: {})", lines.len());
    }
}

fn env_flag(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn build_scene(
    ctx: &nsi::Context<'static>,
    probe: Arc<Probe>,
    image_name: &str,
    start: Instant,
    renderthreads: Option<i32>,
    driver_name: &str,
    denoise: bool,
) {
    if let Some(rt) = renderthreads {
        ctx.set_attribute(".global", &[nsi::i32!("renderthreads", rt)]);
    }
    if !denoise {
        // Interactive renders auto-apply OIDN denoising, which uses the GPU.
        // Disable it so the render is pure-CPU (needed to run in a sandbox
        // where the GPU is unreachable, and to isolate non-OIDN behaviour).
        ctx.set_attribute(".global", &[nsi::i32!("quality.denoise", 0)]);
    }

    // Camera.
    ctx.create("camera_xform", nsi::TRANSFORM, None);
    ctx.connect("camera_xform", None, nsi::ROOT, "objects", None);
    ctx.set_attribute(
        "camera_xform",
        &[nsi::matrix_f64!(
            "transformationmatrix",
            &[
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 5., 1.
            ]
        )],
    );
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, "camera_xform", "objects", None);
    ctx.set_attribute("camera", &[nsi::f32!("fov", 45.)]);

    // Screen + beauty layer + FERRIS driver.
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[
            nsi::i32_slice!("resolution", &[64, 64])
                .array_len(const { NonZeroUsize::new(2).unwrap() }),
            nsi::i32!("oversampling", 4),
        ],
    );

    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    let mut beauty_attrs = vec![
        nsi::string!("variablename", "Ci"),
        nsi::i32!("withalpha", 1),
        nsi::string!("scalarformat", "float"),
        nsi::f64!("filterwidth", 1.0),
    ];
    if !denoise {
        beauty_attrs.push(nsi::i32!("denoise", 0));
    }
    ctx.set_attribute("beauty", &beauty_attrs);
    ctx.connect("beauty", None, "screen", "outputlayers", None);

    let probe_open = Arc::clone(&probe);
    let open = nsi::output::OpenCallback::new(
        move |_name: &str,
              _w: usize,
              _h: usize,
              _fmt: &nsi::output::PixelFormat| {
            probe_open.opened.store(true, Ordering::SeqCst);
            nsi::output::Error::None
        },
    );

    let probe_write = Arc::clone(&probe);
    let write = nsi::output::WriteCallback::<f32>::new(
        move |_name: &str,
              _w: usize,
              _h: usize,
              _x0: usize,
              _x1: usize,
              _y0: usize,
              _y1: usize,
              _fmt: &nsi::output::PixelFormat,
              _bucket: &[f32]| {
            if probe_write.buckets.fetch_add(1, Ordering::SeqCst) == 0 {
                let ms = start.elapsed().as_millis() as u64;
                probe_write.first_bucket_ms.store(ms, Ordering::SeqCst);
            }
            nsi::output::Error::None
        },
    );

    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.connect("driver", None, "beauty", "outputdrivers", None);
    if driver_name.starts_with("ferris") {
        ctx.set_attribute(
            "driver",
            &[
                nsi::string!("drivername", driver_name),
                nsi::string!("imagefilename", image_name),
                nsi::callback!("callback.open", open),
                nsi::callback!("callback.write", write),
            ],
        );
    } else {
        // Built-in 3Delight driver (e.g. "idisplay", "exr"): no callbacks.
        ctx.set_attribute(
            "driver",
            &[
                nsi::string!("drivername", driver_name),
                nsi::string!("imagefilename", image_name),
            ],
        );
    }

    // A self-emitting quad filling the camera view so every bucket is
    // non-empty real geometry (the FERRIS driver clears WantsEmptyBuckets; an
    // empty frame would deliver *no* write callbacks and muddy the
    // experiment). dlConstant self-emits, so no light is needed.
    ctx.create("quad", nsi::MESH, None);
    ctx.connect("quad", None, nsi::ROOT, "objects", None);
    let points: &[[f32; 3]] = &[
        [-2.5, -2.5, 0.],
        [2.5, -2.5, 0.],
        [2.5, 2.5, 0.],
        [-2.5, 2.5, 0.],
    ];
    ctx.set_attribute(
        "quad",
        &[
            nsi::point_slice!("P", points),
            nsi::i32_slice!("P.indices", &[0, 1, 2, 3]),
            nsi::i32_slice!("nvertices", &[4]),
        ],
    );
    ctx.create("quad_attrib", nsi::ATTRIBUTES, None);
    ctx.connect("quad_attrib", None, "quad", "geometryattributes", None);
    ctx.create("quad_shader", nsi::SHADER, None);
    ctx.connect("quad_shader", None, "quad_attrib", "surfaceshader", None);
    ctx.set_attribute(
        "quad_shader",
        &[
            nsi::string!("shaderfilename", "${DELIGHT}/osl/dlConstant"),
            nsi::color!("i_color", &[0.2, 0.5, 0.8]),
            nsi::f32!("intensity", 1.),
        ],
    );
}

/// Build a context (with an error handler) + scene, then run the full render
/// lifecycle. Returns once `Start` (+ `Synchronize`) is issued via `started`,
/// then blocks until `stop_rx` fires, runs `Stop` + `Wait`, and signals `done`.
// One parameter per moving part of a context's lifecycle; bundling them into
// a struct would only move the same list one level down.
#[allow(clippy::too_many_arguments)]
fn drive_context(
    label: String,
    image_name: String,
    probe: Arc<Probe>,
    start: Instant,
    cfg: Config,
    started: mpsc::Sender<()>,
    stop_rx: mpsc::Receiver<()>,
    done: mpsc::Sender<()>,
) {
    let err_label = label.clone();
    let error_handler = nsi::ErrorCallback::new(
        move |level: log::Level, id: i32, msg: &str| {
            println!("  {err_label} NSI {level:?} [{id}]: {msg}");
        },
    );
    let ctx = nsi::Context::new(Some(&[nsi::callback!(
        "errorhandler",
        error_handler
    )]))
    .expect("Could not create NSI context");

    build_scene(
        &ctx,
        probe,
        &image_name,
        start,
        cfg.renderthreads,
        cfg.driver_name,
        cfg.denoise,
    );

    let status_label = label.clone();
    let status = nsi::StatusCallback::new(
        move |_ctx: &nsi::Context, status: nsi::RenderStatus| {
            println!(
                "  {status_label} status {status:?} @ {}ms",
                start.elapsed().as_millis()
            );
        },
    );

    let mut args = Vec::new();
    if cfg.use_status_callback {
        args.push(nsi::callback!("callback", status));
    }
    if cfg.interactive {
        args.push(nsi::i32!("interactive", 1));
        args.push(nsi::i32!("progressive", 1));
    }
    ctx.render_control(nsi::Action::Start, Some(&args));
    if cfg.interactive && cfg.do_sync {
        ctx.render_control(nsi::Action::Synchronize, None);
    }
    let _ = started.send(());

    // For a batch render, Wait here so it actually completes.
    if !cfg.interactive {
        ctx.render_control(nsi::Action::Wait, None);
    }

    // Block until told to tear down (buckets flow on 3Delight's threads).
    // While waiting, periodically Synchronize like akatela's render thread does
    // (in case a progressive render only advances on repeated Synchronize).
    loop {
        match stop_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if cfg.interactive && cfg.do_sync {
                    ctx.render_control(nsi::Action::Synchronize, None);
                }
            }
        }
    }

    if cfg.interactive {
        ctx.render_control(nsi::Action::Stop, None);
        ctx.render_control(nsi::Action::Wait, None);
    }
    let _ = done.send(());
    // ctx dropped here -> NSIEnd.
}

fn main() -> ExitCode {
    // Block SIGUSR1 in the main thread *before* any other thread (ours or
    // 3Delight's) is spawned, so they all inherit the block. 3Delight runs a
    // dedicated thread that `sigwait`s on SIGUSR1 to dump stack traces; if any
    // thread leaves SIGUSR1 unblocked the kernel may deliver it there and the
    // default action terminates the process instead. With it blocked
    // everywhere, `kill(pid, SIGUSR1)` stays pending until 3Delight's waiter
    // consumes it. (Harmless when NSI_SIGUSR1 is unset.)
    if env_flag("NSI_SIGUSR1", false) && env_flag("NSI_SIGUSR1_BLOCK", false) {
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            libc::sigaddset(&mut set, libc::SIGUSR1);
            libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        }
    }

    let count = env_usize("NSI_CTX_COUNT", 2);
    let cfg = Config {
        interactive: env_flag("NSI_INTERACTIVE", true),
        unique_names: env_flag("NSI_UNIQUE_NAMES", false),
        do_sync: env_flag("NSI_SYNC", true),
        use_status_callback: env_flag("NSI_STATUS", true),
        renderthreads: env::var("NSI_RENDERTHREADS")
            .ok()
            .and_then(|v| v.parse::<i32>().ok()),
        driver_name: match env::var("NSI_DRIVER").ok().as_deref() {
            Some("idisplay") => "idisplay",
            Some("exr") => "exr",
            Some("file") => "file",
            _ => nsi::output::FERRIS_F32,
        },
        denoise: env_flag("NSI_DENOISE", true),
    };
    let drive_main =
        env::var("NSI_DRIVE").map(|v| v == "main").unwrap_or(false);
    let first_bucket_timeout =
        Duration::from_secs(env_usize("NSI_FIRST_BUCKET_TIMEOUT", 20) as u64);
    let teardown_timeout =
        Duration::from_secs(env_usize("NSI_TEARDOWN_TIMEOUT", 8) as u64);

    logln(&format!(
        "config: count={count} interactive={} unique_names={} sync={} \
         status_cb={} drive={} denoise={} driver={} renderthreads={:?}",
        cfg.interactive,
        cfg.unique_names,
        cfg.do_sync,
        cfg.use_status_callback,
        if drive_main { "main" } else { "thread" },
        cfg.denoise,
        cfg.driver_name,
        cfg.renderthreads
    ));

    let start = Instant::now();
    let mut probes: Vec<Arc<Probe>> = Vec::new();
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();
    let mut stop_txs: Vec<mpsc::Sender<()>> = Vec::new();
    let mut done_rxs: Vec<mpsc::Receiver<()>> = Vec::new();

    for i in 0..count {
        let label = format!("NSI[{i}]");
        let probe = Probe::new(label.clone());
        let name = if cfg.unique_names {
            format!("repro_{i}")
        } else {
            "repro_shared".to_string()
        };
        probes.push(Arc::clone(&probe));

        let (started_tx, started_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        stop_txs.push(stop_tx);
        done_rxs.push(done_rx);

        let drive = move || {
            drive_context(
                label, name, probe, start, cfg, started_tx, stop_rx, done_tx,
            )
        };

        let _ = drive_main;
        handles.push(thread::spawn(drive));
        // Wait until this context has issued Start.
        let _ = started_rx.recv();
        logln(&format!(
            "NSI[{i}] started ({}) imagefilename={}",
            if cfg.interactive {
                "interactive+progressive"
            } else {
                "batch"
            },
            if cfg.unique_names {
                format!("repro_{i}")
            } else {
                "repro_shared".to_string()
            }
        ));
    }

    // Wait for the first bucket on every context.
    let deadline = Instant::now() + first_bucket_timeout;
    loop {
        let all = probes.iter().all(|p| p.buckets.load(Ordering::SeqCst) > 0);
        if all || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    // If the render is hung (no buckets) and NSI_SIGUSR1 is set, send SIGUSR1
    // to ourselves: 3Delight installs a SIGUSR1 handler that dumps a full set
    // of thread stack traces (to stderr), which is exactly what's needed to
    // diagnose where it is wedged (e.g. a stuck GPU/OIDN call).
    if env_flag("NSI_SIGUSR1", false)
        && probes.iter().any(|p| p.buckets.load(Ordering::SeqCst) == 0)
    {
        logln(
            ">>> render appears hung — sending SIGUSR1 for 3Delight stack dump",
        );
        let _ = std::io::stdout().flush();
        // SAFETY: raising a signal to our own process is sound; 3Delight's
        // handler dumps stacks and returns.
        unsafe {
            libc::kill(libc::getpid(), libc::SIGUSR1);
        }
        // Give 3Delight time to emit the (large) dump before we continue.
        thread::sleep(Duration::from_secs(5));
        logln(">>> SIGUSR1 dump window elapsed");
    }

    logln("--- first-bucket results ---");
    let mut any_no_pixels = false;
    for p in &probes {
        let buckets = p.buckets.load(Ordering::SeqCst);
        let first = p.first_bucket_ms.load(Ordering::SeqCst);
        let opened = p.opened.load(Ordering::SeqCst);
        let first_str = if first == u64::MAX {
            "NEVER".to_string()
        } else {
            format!("{first}ms")
        };
        if buckets == 0 {
            any_no_pixels = true;
        }
        logln(&format!(
            "{}: opened={opened} buckets={buckets} first_bucket={first_str}",
            p.label
        ));
    }

    if env_flag("NSI_DUMP_THREADS", false) {
        dump_self_threads("after bucket poll (render live)");
    }

    // Tear every context down (signal stop) and wait with a watchdog.
    logln("--- teardown (Stop + Wait) ---");
    for tx in &stop_txs {
        let _ = tx.send(());
    }
    if env_flag("NSI_DUMP_THREADS", false) {
        thread::sleep(Duration::from_secs(2));
        dump_self_threads("during Stop+Wait (mid-teardown)");
    }
    let mut any_deadlock = false;
    for (i, rx) in done_rxs.iter().enumerate() {
        match rx.recv_timeout(teardown_timeout) {
            Ok(()) => logln(&format!("NSI[{i}]: teardown OK")),
            Err(_) => {
                any_deadlock = true;
                logln(&format!(
                    "NSI[{i}]: DEADLOCK — Stop+Wait did not return in {teardown_timeout:?}"
                ));
            }
        }
    }

    logln("=== SUMMARY ===");
    logln(&format!("no-pixels failure: {any_no_pixels}"));
    logln(&format!("teardown deadlock: {any_deadlock}"));
    if any_no_pixels || any_deadlock {
        logln("RESULT: FAIL (reproduced the concurrent-interactive bug)");
    } else {
        logln("RESULT: PASS (all contexts produced pixels and tore down)");
    }

    // Join the render threads.
    //
    // The `done` message above only says the thread reached the end of its
    // work -- it has NOT exited yet: it still has to drop its `Context`,
    // which is what runs `NSIEnd`. Returning from `main` at that point
    // leaves threads inside 3Delight while libc tears the process down,
    // which is a SIGSEGV *after* a clean render. Measured: not joining
    // crashed ~5/6 runs; joining does not crash.
    //
    // This is why the same shape in pure C never crashed -- it
    // `pthread_join`s.
    for handle in handles {
        let _ = handle.join();
    }

    // Escape hatch kept for measuring the old behaviour: any non-zero value
    // sleeps instead, which is what used to paper over the missing join.
    if let Ok(ms) = std::env::var("NSI_EXIT_DELAY_MS")
        && let Ok(ms) = ms.parse::<u64>()
        && ms > 0
    {
        thread::sleep(Duration::from_millis(ms));
    }

    let _ = std::io::stdout().flush();

    // Deliberately NOT `std::process::exit()`. That skips every destructor,
    // so a live `Context` never runs `NSIEnd` at all. Returning `ExitCode`
    // gives the same exit status with destructors intact.
    if any_no_pixels || any_deadlock {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
