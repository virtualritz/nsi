use crate::{Descriptor, Error, Level, Params, Procedural, Report, execute};
use nsi_ffi_wrap::{self as nsi, Arg, Nsi, ParamValue, Type};
use std::{num::NonZeroUsize, sync::Mutex};

/// The C views of `args`, which borrow from them.
fn raw(args: &[Arg<'_, '_>]) -> Vec<nsi::FfiParam> {
    args.iter()
        .map(|arg| arg.as_c_param().expect("an Arg has a C view"))
        .collect()
}

fn params<'a>(raw: &'a [nsi::FfiParam]) -> Params<'a> {
    // SAFETY: built from live `Arg`s by `raw`.
    unsafe { Params::from_raw(raw.as_ptr(), raw.len() as _) }
}

#[test]
fn every_type_reads_back_what_was_written() {
    let matrix = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 3.0,
        4.0, 1.0,
    ];
    let args = [
        nsi::real_f32!("f32", 0.5),
        nsi::real_f64!("f64", 0.25),
        nsi::integer_i32!("i32", -7),
        nsi::integer_i64!("i64", 1 << 40),
        nsi::string!("string", "hello"),
        nsi::color3_f32!("color", &[0.1, 0.2, 0.3]),
        nsi::matrix4_f64!("matrix", &matrix),
        nsi::string_slice!("strings", &["a", "b", "c"]),
        nsi::integer_i32_slice!("pairs", &[1, 2, 3, 4, 5, 6])
            .array_len(NonZeroUsize::new(2).unwrap()),
    ];
    let raw = raw(&args);
    let params = params(&raw);

    assert_eq!(params.len(), args.len());
    assert_eq!(params.get("f32").and_then(|p| p.f32()), Some(0.5));
    assert_eq!(params.get("f64").and_then(|p| p.f64()), Some(0.25));
    assert_eq!(params.get("i32").and_then(|p| p.i32()), Some(-7));
    assert_eq!(
        params.get("i64").and_then(|p| p.i64s()),
        Some(&[1_i64 << 40][..])
    );
    assert_eq!(
        params.get("string").and_then(|p| p.string()),
        Some(c"hello")
    );

    let color = params.get("color").unwrap();
    assert_eq!(color.type_tag(), Some(Type::Color3F32));
    assert_eq!(color.f32s(), Some(&[0.1, 0.2, 0.3][..]));

    assert_eq!(
        params.get("matrix").and_then(|p| p.f64s()),
        Some(&matrix[..])
    );

    let strings = params
        .get("strings")
        .and_then(|p| p.strings())
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(strings, [c"a", c"b", c"c"]);

    // An array type: three values of two elements each, not six values.
    let pairs = params.get("pairs").unwrap();
    assert_eq!(pairs.count(), 3);
    assert_eq!(pairs.array_length(), 2);
    assert_eq!(pairs.i32s(), Some(&[1, 2, 3, 4, 5, 6][..]));
}

#[test]
fn the_wrong_type_is_a_miss_not_a_reinterpretation() {
    let args = [nsi::integer_i32!("count", 3), nsi::real_f64!("scale", 2.0)];
    let raw = raw(&args);
    let params = params(&raw);

    assert_eq!(params.get("count").and_then(|p| p.f32()), None);
    assert!(params.get("count").and_then(|p| p.strings()).is_none());
    assert_eq!(params.get("scale").and_then(|p| p.f32s()), None);
    assert!(params.get("missing").is_none());
}

#[test]
fn no_parameters_is_empty_not_undefined() {
    // SAFETY: a null array with a zero count is what the renderer passes
    // for a call without parameters.
    let params = unsafe { Params::from_raw(core::ptr::null(), 0) };
    assert!(params.is_empty());
    assert!(params.get("anything").is_none());
}

#[test]
fn the_descriptor_is_laid_out_as_nsi_procedural_t() {
    assert_eq!(
        core::mem::size_of::<Descriptor>(),
        core::mem::size_of::<nsi::nsi_sys::NSIProcedural>()
    );
    assert_eq!(core::mem::offset_of!(Descriptor, nsi_version), 0);
}

#[test]
fn levels_are_nsi_error_levels() {
    use nsi::nsi_sys::NSIErrorLevel;
    assert_eq!(Level::Message as i32, NSIErrorLevel::Message as i32);
    assert_eq!(Level::Info as i32, NSIErrorLevel::Info as i32);
    assert_eq!(Level::Warning as i32, NSIErrorLevel::Warning as i32);
    assert_eq!(Level::Error as i32, NSIErrorLevel::Error as i32);
}

/// Creates `count` spheres, and reports how many.
struct Spheres;

impl Procedural for Spheres {
    fn load(_: &Report<'_>, _: &str) -> Result<Self, Error> {
        Ok(Spheres)
    }

    fn execute<N>(
        &self,
        nsi: &N,
        report: &Report<'_>,
        params: Params<'_>,
    ) -> Result<(), Error>
    where
        for<'call> N: Nsi<Arg<'call> = Arg<'call, 'static>>,
    {
        let count = params.get("count").and_then(|p| p.i32()).unwrap_or(1);
        for index in 0..count {
            let handle = format!("sphere{index}");
            nsi.create(&handle, nsi::PARTICLES, None)?;
            nsi.connect(&handle, None, nsi::ROOT, "objects", None)?;
        }
        report.info(&format!("created {count}"));
        Ok(())
    }
}

#[test]
fn a_compiled_in_procedural_runs_on_the_hosts_own_nsi() {
    let recorder = nsi_intermediate::Recorder::new();
    let messages = Mutex::new(Vec::new());
    let sink = |level: Level, message: &str| {
        messages.lock().unwrap().push((level, message.to_string()))
    };

    let procedural = Spheres::load(&Report::new(&sink), "test").unwrap();
    execute(
        &procedural,
        &recorder,
        &Report::new(&sink),
        &[nsi::integer_i32!("count", 3)],
    )
    .unwrap();

    let scene = recorder.into_scene();
    let spheres = ["sphere0", "sphere1", "sphere2"];
    assert!(spheres.iter().all(|handle| {
        scene.node(handle).map(|node| node.node_type()) == Some(nsi::PARTICLES)
    }));
    assert!(scene.node("sphere3").is_none());
    assert_eq!(
        messages.into_inner().unwrap(),
        [(Level::Info, "created 3".to_string())]
    );
}

#[test]
fn a_hosts_error_reaches_the_caller() {
    let recorder = nsi_intermediate::Recorder::new();
    let sink = |_: Level, _: &str| {};
    // `sphere0` exists as a mesh, so creating it as a sphere is refused.
    recorder.create("sphere0", nsi::MESH, None).unwrap();

    let result = execute(&Spheres, &recorder, &Report::new(&sink), &[]);
    assert!(result.is_err());
}

/// What the stand-in renderer's `report` was handed, per context.
static REPORTED: Mutex<Vec<(i32, i32, String)>> = Mutex::new(Vec::new());

/// The renderer's `NSIReport_t`, standing in for one.
unsafe extern "C" fn record_report(
    context: core::ffi::c_int,
    level: core::ffi::c_int,
    message: *const core::ffi::c_char,
) {
    // SAFETY: the shim passes a NUL-terminated message.
    let message = unsafe { core::ffi::CStr::from_ptr(message) };
    REPORTED.lock().unwrap().push((
        context,
        level,
        message.to_string_lossy().into_owned(),
    ));
}

/// The messages reported on `context`.
fn reported(context: i32) -> Vec<(i32, String)> {
    REPORTED
        .lock()
        .unwrap()
        .iter()
        .filter(|(on, _, _)| *on == context)
        .map(|(_, level, message)| (*level, message.clone()))
        .collect()
}

/// Fails to load: `Err` or panic, by what the version says.
struct Refuses;

impl Procedural for Refuses {
    fn load(_: &Report<'_>, renderer_version: &str) -> Result<Self, Error> {
        match renderer_version {
            "panic" => panic!("refusing loudly"),
            _ => Err("refusing politely".into()),
        }
    }

    fn execute<N>(
        &self,
        _: &N,
        _: &Report<'_>,
        _: Params<'_>,
    ) -> Result<(), Error>
    where
        for<'call> N: Nsi<Arg<'call> = Arg<'call, 'static>>,
    {
        unreachable!("never loads")
    }
}

/// Calls the exported entry point's body as a renderer would.
fn load<P: Procedural>(context: i32, version: &core::ffi::CStr) -> bool {
    // SAFETY: valid strings, and a report function that reads them.
    let descriptor = unsafe {
        crate::shim_load::<P>(
            context,
            Some(record_report),
            c"lib3delight.so".as_ptr(),
            version.as_ptr(),
        )
    };
    !descriptor.is_null()
}

#[test]
fn a_load_error_is_reported_and_loads_nothing() {
    assert!(!load::<Refuses>(101, c"2.9"));
    assert_eq!(
        reported(101),
        [(
            Level::Error as i32,
            "procedural failed to load: refusing politely".to_string()
        )]
    );
}

#[test]
fn a_load_panic_is_reported_not_unwound_into_c() {
    assert!(!load::<Refuses>(102, c"panic"));
    assert_eq!(
        reported(102),
        [(
            Level::Error as i32,
            "procedural panicked while loading: refusing loudly".to_string()
        )]
    );
}

#[test]
fn a_missing_library_path_is_reported() {
    // SAFETY: a null path is what is under test; the version is valid.
    let descriptor = unsafe {
        crate::shim_load::<Spheres>(
            103,
            Some(record_report),
            core::ptr::null(),
            c"2.9".as_ptr(),
        )
    };
    assert!(descriptor.is_null());
    assert_eq!(
        reported(103),
        [(
            Level::Error as i32,
            "the renderer passed no ɴsɪ library path".to_string()
        )]
    );
}
