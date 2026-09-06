# `nsi-display` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A crate that lets someone write a 3Delight display driver as a
safe Rust type plus one macro invocation, instead of hand-written
`extern "C"` shims.

**Architecture:** The author implements a `DisplayDriver` trait; the
`declare_display_driver!` macro emits the four `DspyImage*` symbols the
renderer resolves by name. The shims own the driver state behind
`PtDspyImageHandle`, treat all renderer-supplied pointers as borrowed,
and convert panics into error codes. Pixel-format parsing and the pixel
scalar types are reused from `nsi_ffi_wrap::output`.

**Tech Stack:** Rust 2024, `ndspy-sys` 0.2, `nsi-ffi-wrap` (feature
`output`), Miri for the shim proofs, 3Delight 2.9 for the end-to-end test.

**Spec:** `docs/superpowers/specs/2026-09-05-nsi-plugin-crates-design.md`

## Global Constraints

- `rust-version = "1.88"`, `edition = "2024"` — matches the workspace.
- `license = "MIT OR Apache-2.0 OR Zlib"`, `repository = "https://github.com/virtualritz/nsi/"`.
- `#[unsafe(no_mangle)]`, not `#[no_mangle]` — required by edition 2024.
- Every generated shim wraps author code in `catch_unwind`. A panic
  becomes an error code, never an unwind into C.
- Renderer-supplied pointers (`UserParameter*`, `PtDspyDevFormat*`,
  `const unsigned char*`) are **borrowed for the call only**. Never
  `Box::from_raw` one.
- `PtDspyImageHandle` is **ours**: `Box::into_raw` in open, `Box::from_raw`
  exactly once in close.
- `cargo fmt --all --check`, and `cargo clippy --workspace --all-targets
--features output -- -D warnings` must stay clean.
- Running the 3Delight-backed tests needs the licence server up:
  `setsid $DELIGHT/bin/licserver -d $DELIGHT/license.dat </dev/null &`

## Deviation from the spec, decided here

The spec sketches `const MULTITHREAD: bool` selecting between
`write(&mut self)` and `write(&self) + Sync`. **v1 implements the
serialised case only**: `write(&mut self)`, and `PkThreadQuery` answered
with `multithread = 0`. That is the standard ndspy contract (the query is
a 3Delight extension), and it is what makes `&mut self` sound with no
locking. A `ConcurrentDisplayDriver` trait for the opt-in case is future
work, deliberately not built until someone needs it. One trait, no
conditional bounds, no `Mutex` imposed on the common file-writing driver.

## Prior art: `r-display`

`github.com/virtualritz/r-display`, vendored at
`crates/ndspy-sys/examples/r-display`, is a working 3Delight display
driver in Rust written by hand. It is the crate's motivation, and the
audit of it is the acceptance criterion: each of the following must be
impossible to write with `nsi-display`.

| r-display | Where |
| --- | --- |
| `PkSizeQuery` `Box::from_raw`s the image handle and drops it -- freeing memory the renderer still holds | `lib.rs:156` |
| `DspyImageData` `Box::from_raw`s *before* the null check; a null handle is instant UB and any early return frees the handle | `:201` |
| `DspyImageQuery` assigns `Box::into_raw(..)` to the by-value `data` parameter, so both answers are discarded and leaked | `:168`, `:180` |
| `get_parameter<T>` ignores `valueType` -- type confusion by construction | `:41` |
| No `catch_unwind`; `.to_str().unwrap()` unwinds into C on bad UTF-8 | throughout |
| `copy_nonoverlapping` at a running offset, never bounds-checked | `:209` |

Task 7 ports it. The port is the example *and* the proof: the same
driver, with none of the above expressible.

## File Structure

| File                                        | Responsibility                                            |
| ------------------------------------------- | --------------------------------------------------------- |
| `crates/nsi-display/Cargo.toml`             | Manifest, publish metadata                                |
| `crates/nsi-display/src/lib.rs`             | Crate docs, re-exports, `DisplayDriver` trait             |
| `crates/nsi-display/src/params.rs`          | `Params<'_>` — borrowed view over `UserParameter[]`       |
| `crates/nsi-display/src/bucket.rs`          | `Bucket<'_, T>` — one region of pixels                    |
| `crates/nsi-display/src/shim.rs`            | `declare_display_driver!` and the generic bodies it calls |
| `crates/nsi-display/tests/shims.rs`         | Miri-checkable tests driving the generated symbols        |
| `crates/nsi-display/examples/ppm_driver.rs` | A worked driver, built as a cdylib                        |
| `crates/nsi-display/README.md`              | Crate README                                              |

The macro delegates to non-generated generic functions in `shim.rs` so
the emitted code stays a few lines per symbol. Macro-heavy code is hard to
read in expansion; keeping the bodies in real functions means they get
type-checked and tested normally.

---

### Task 1: Make `PixelFormat` constructible outside `nsi-ffi-wrap`

`nsi-display` must turn the renderer's `PtDspyDevFormat[]` into a
`PixelFormat`. `PixelFormat::new` is `pub(crate)` today, so it cannot.

**Files:**

- Modify: `crates/nsi-ffi-wrap/src/output/pixel_format.rs:234`
- Test: `crates/nsi-ffi-wrap/tests/pixel_format_public.rs` (create)

**Interfaces:**

- Produces: `nsi_ffi_wrap::output::PixelFormat::from_ndspy(&[ndspy_sys::PtDspyDevFormat]) -> PixelFormat`

- [ ] **Step 1: Write the failing test**

```rust
//! `PixelFormat` must be constructible by out-of-crate display drivers.
use nsi_ffi_wrap::output::PixelFormat;
use std::ffi::CString;

#[test]
fn a_display_driver_can_build_a_pixel_format_from_ndspy() {
    let name = CString::new("beauty.r").unwrap();
    let format = [ndspy_sys::PtDspyDevFormat {
        name: name.as_ptr(),
        type_: 1, // PkDspyFloat32
    }];

    let pixel_format = PixelFormat::from_ndspy(&format);

    assert_eq!(1, pixel_format.len());
    assert_eq!(1, pixel_format.channels());
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nsi-ffi-wrap --features output --test pixel_format_public`
Expected: FAIL to compile — `from_ndspy` does not exist.

- [ ] **Step 3: Add the public constructor**

In `pixel_format.rs`, keep `new` as-is and add beside it:

```rust
    /// Builds a `PixelFormat` from the format array ndspy hands a
    /// display driver.
    ///
    /// Out-of-crate drivers (see the `nsi-display` crate) receive
    /// `PtDspyDevFormat[]` in `DspyImageOpen` and need this to interpret
    /// the buckets that follow.
    #[inline]
    pub fn from_ndspy(format: &[ndspy_sys::PtDspyDevFormat]) -> Self {
        Self::new(format)
    }
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p nsi-ffi-wrap --features output --test pixel_format_public`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/nsi-ffi-wrap/src/output/pixel_format.rs \
        crates/nsi-ffi-wrap/tests/pixel_format_public.rs
git commit -m "Expose PixelFormat::from_ndspy for out-of-crate drivers"
```

---

### Task 2: Crate skeleton and `Error`

**Files:**

- Create: `crates/nsi-display/Cargo.toml`, `crates/nsi-display/src/lib.rs`
- Test: inside `src/lib.rs`

**Interfaces:**

- Consumes: nothing.
- Produces: crate `nsi_display`; `nsi_display::Error` (re-export of
  `nsi_ffi_wrap::output::Error`); `nsi_display::Result<T> = core::result::Result<T, Error>`.

- [ ] **Step 1: Write the manifest**

`crates/nsi-display/Cargo.toml`:

```toml
[package]
name = "nsi-display"
version = "0.1.0"
authors = ["Moritz Moeller <virtualritz@protonmail.com>"]
edition = "2024"
# Let-chains, stable for edition 2024 since 1.88.
rust-version = "1.88"
keywords = ["graphics", "rendering", "3d", "display-driver", "plugin"]
categories = ["graphics", "multimedia::images", "rendering::graphics-api"]
license = "MIT OR Apache-2.0 OR Zlib"
description = "Write ɴsɪ display drivers in safe Rust."
readme = "README.md"
documentation = "https://docs.rs/nsi-display/"
repository = "https://github.com/virtualritz/nsi/"

[dependencies]
ndspy-sys = "0.2"
nsi-ffi-wrap = { version = "0.9", path = "../nsi-ffi-wrap", features = ["output"] }

[package.metadata.docs.rs]
all-features = true
```

- [ ] **Step 2: Write the failing test**

`crates/nsi-display/src/lib.rs`:

```rust
//! Write ɴsɪ display drivers in safe Rust.

pub use nsi_ffi_wrap::output::{Error, PixelFormat, PixelType};

/// The result an author's driver methods return.
pub type Result<T> = core::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The error type must round-trip to the code ndspy expects, since
    /// that is what every generated shim returns.
    #[test]
    fn errors_map_to_ndspy_codes() {
        assert_eq!(
            ndspy_sys::PtDspyError::None as u32,
            u32::from(Error::None)
        );
        assert_eq!(
            ndspy_sys::PtDspyError::BadParams as u32,
            u32::from(Error::BadParameters)
        );
    }
}
```

- [ ] **Step 3: Add the crate to the workspace and run the test**

The workspace `members = ["crates/*"]` already picks it up.

Run: `cargo test -p nsi-display`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/nsi-display/
git commit -m "Add the nsi-display crate skeleton"
```

---

### Task 3: `Params` — a borrowed view over the renderer's parameters

**Files:**

- Create: `crates/nsi-display/src/params.rs`
- Modify: `crates/nsi-display/src/lib.rs`

**Interfaces:**

- Produces:
  - `Params<'a>` with `unsafe fn from_raw(*const ndspy_sys::UserParameter, c_int) -> Params<'a>`
  - `Params::string(&self, name: &str) -> Option<&'a str>`
  - `Params::i32(&self, name: &str) -> Option<i32>`
  - `Params::f32(&self, name: &str) -> Option<f32>`
  - `Params::len(&self) -> usize`, `Params::is_empty(&self) -> bool`

- [ ] **Step 1: Write the failing test**

At the bottom of `params.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// A `Params` view borrows the renderer's array; it must read the
    /// values back without taking ownership of anything.
    #[test]
    fn reads_string_and_integer_parameters() {
        let name = CString::new("filename").unwrap();
        let value = CString::new("render.exr").unwrap();
        let value_ptr = value.as_ptr();
        let quality_name = CString::new("quality").unwrap();
        let quality = 42i32;

        let raw = [
            ndspy_sys::UserParameter {
                name: name.as_ptr(),
                valueType: b's' as _,
                valueCount: 1,
                value: &value_ptr as *const _ as *const c_void,
                nbytes: core::mem::size_of::<*const c_char>() as _,
            },
            ndspy_sys::UserParameter {
                name: quality_name.as_ptr(),
                valueType: b'i' as _,
                valueCount: 1,
                value: &quality as *const _ as *const c_void,
                nbytes: core::mem::size_of::<i32>() as _,
            },
        ];

        // SAFETY: `raw` outlives the view.
        let params = unsafe { Params::from_raw(raw.as_ptr(), raw.len() as _) };

        assert_eq!(2, params.len());
        assert_eq!(Some("render.exr"), params.string("filename"));
        assert_eq!(Some(42), params.i32("quality"));
        assert_eq!(None, params.i32("absent"));
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nsi-display params`
Expected: FAIL to compile — `Params` does not exist.

- [ ] **Step 3: Implement `Params`**

`crates/nsi-display/src/params.rs`:

```rust
//! A borrowed view over the parameters ndspy hands a driver.

use core::{
    ffi::{CStr, c_char, c_int, c_void},
    marker::PhantomData,
    slice,
};

/// The parameters the renderer passes to `DspyImageOpen`.
///
/// Borrowed, never owned: the array belongs to the renderer and is valid
/// only for the duration of the call. Copy anything you need to keep.
#[derive(Copy, Clone)]
pub struct Params<'a> {
    raw: &'a [ndspy_sys::UserParameter],
    _marker: PhantomData<&'a ()>,
}

impl<'a> Params<'a> {
    /// # Safety
    /// `raw` must point to `count` valid `UserParameter`s that outlive
    /// `'a`, as ndspy guarantees for the duration of the call.
    #[inline]
    pub unsafe fn from_raw(
        raw: *const ndspy_sys::UserParameter,
        count: c_int,
    ) -> Self {
        let raw = if raw.is_null() || count <= 0 {
            &[][..]
        } else {
            // SAFETY: the caller guarantees `count` valid entries.
            unsafe { slice::from_raw_parts(raw, count as usize) }
        };
        Self { raw, _marker: PhantomData }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    fn find(&self, name: &str, type_: u8) -> Option<&'a ndspy_sys::UserParameter> {
        self.raw.iter().find(|p| {
            if p.name.is_null() || p.value.is_null() || p.valueType as u8 != type_
            {
                return false;
            }
            // SAFETY: ndspy names are NUL-terminated C strings.
            unsafe { CStr::from_ptr(p.name) }.to_str() == Ok(name)
        })
    }

    /// A string parameter. ndspy stores these as a pointer to a
    /// `char*`, so this reads through two levels, exactly as the value
    /// is laid out.
    pub fn string(&self, name: &str) -> Option<&'a str> {
        let param = self.find(name, b's')?;
        // SAFETY: `value` addresses one `char*`, per the ndspy layout.
        let ptr = unsafe { *(param.value as *const *const c_char) };
        if ptr.is_null() {
            return None;
        }
        // SAFETY: the renderer passes NUL-terminated strings.
        unsafe { CStr::from_ptr(ptr) }.to_str().ok()
    }

    /// An integer parameter.
    pub fn i32(&self, name: &str) -> Option<i32> {
        let param = self.find(name, b'i')?;
        // SAFETY: `value` addresses one `int`.
        Some(unsafe { *(param.value as *const i32) })
    }

    /// A float parameter.
    pub fn f32(&self, name: &str) -> Option<f32> {
        let param = self.find(name, b'f')?;
        // SAFETY: `value` addresses one `float`.
        Some(unsafe { *(param.value as *const f32) })
    }
}
```

Add to `lib.rs`:

```rust
mod params;
pub use params::Params;
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p nsi-display params`
Expected: PASS

- [ ] **Step 5: Run it under Miri**

Run: `cargo +nightly miri test -p nsi-display params`
Expected: PASS, no leaks — the view must not have taken ownership.

- [ ] **Step 6: Commit**

```bash
git add crates/nsi-display/src/params.rs crates/nsi-display/src/lib.rs
git commit -m "Add a borrowed Params view over ndspy's UserParameter array"
```

---

### Task 4: `Bucket` — one region of pixels

**Files:**

- Create: `crates/nsi-display/src/bucket.rs`
- Modify: `crates/nsi-display/src/lib.rs`

**Interfaces:**

- Produces:
  - `Bucket<'a, T: PixelType>` with public accessors `x_min`, `x_max`,
    `y_min`, `y_max`, `width`, `height`, `channels`, `pixels() -> &'a [T]`
  - `Bucket::new(x_min: usize, x_max: usize, y_min: usize, y_max: usize, channels: usize, pixels: &'a [T]) -> Self`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The pixel slice is exactly the announced region, channels
    /// included. Getting this wrong is a buffer overread, so the
    /// constructor asserts it.
    #[test]
    fn geometry_matches_the_pixel_slice() {
        let pixels = [0.5f32; 2 * 3 * 4]; // 2x3 pixels, 4 channels
        let bucket = Bucket::new(0, 2, 0, 3, 4, &pixels);

        assert_eq!(2, bucket.width());
        assert_eq!(3, bucket.height());
        assert_eq!(4, bucket.channels());
        assert_eq!(pixels.len(), bucket.pixels().len());
    }

    #[test]
    #[should_panic(expected = "bucket geometry")]
    fn a_mismatched_slice_is_rejected() {
        let pixels = [0.5f32; 3];
        let _ = Bucket::new(0, 2, 0, 3, 4, &pixels);
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nsi-display bucket`
Expected: FAIL to compile — `Bucket` does not exist.

- [ ] **Step 3: Implement `Bucket`**

```rust
//! One region of pixels, as handed to `DspyImageData`.

use nsi_ffi_wrap::output::PixelType;

/// A rectangular region of the image, with its pixel data.
///
/// The data is the **bucket only**, not the full image, and is borrowed
/// from the renderer for the duration of the call.
#[derive(Copy, Clone)]
pub struct Bucket<'a, T: PixelType> {
    x_min: usize,
    x_max: usize,
    y_min: usize,
    y_max: usize,
    channels: usize,
    pixels: &'a [T],
}

impl<'a, T: PixelType> Bucket<'a, T> {
    /// # Panics
    /// If `pixels` is not exactly `width * height * channels` long.
    /// The shim computes it from the same numbers it passes here, so a
    /// mismatch is a bug in this crate, not in the author's driver.
    pub fn new(
        x_min: usize,
        x_max: usize,
        y_min: usize,
        y_max: usize,
        channels: usize,
        pixels: &'a [T],
    ) -> Self {
        let expected = (x_max - x_min) * (y_max - y_min) * channels;
        assert_eq!(
            expected,
            pixels.len(),
            "bucket geometry disagrees with the pixel slice"
        );
        Self { x_min, x_max, y_min, y_max, channels, pixels }
    }

    #[inline]
    pub fn x_min(&self) -> usize { self.x_min }
    #[inline]
    pub fn x_max(&self) -> usize { self.x_max }
    #[inline]
    pub fn y_min(&self) -> usize { self.y_min }
    #[inline]
    pub fn y_max(&self) -> usize { self.y_max }
    #[inline]
    pub fn width(&self) -> usize { self.x_max - self.x_min }
    #[inline]
    pub fn height(&self) -> usize { self.y_max - self.y_min }
    #[inline]
    pub fn channels(&self) -> usize { self.channels }
    #[inline]
    pub fn pixels(&self) -> &'a [T] { self.pixels }
}
```

Add to `lib.rs`:

```rust
mod bucket;
pub use bucket::Bucket;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nsi-display bucket`
Expected: PASS, both tests.

- [ ] **Step 5: Commit**

```bash
git add crates/nsi-display/src/bucket.rs crates/nsi-display/src/lib.rs
git commit -m "Add the Bucket type"
```

---

### Task 5: The `DisplayDriver` trait and the shim bodies

**Files:**

- Create: `crates/nsi-display/src/shim.rs`
- Modify: `crates/nsi-display/src/lib.rs`

**Interfaces:**

- Consumes: `Params`, `Bucket`, `Error`, `PixelFormat`, `PixelType`.
- Produces:
  - `trait DisplayDriver: Sized + 'static { type Pixel: PixelType; fn open(...) -> Result<Self>; fn write(&mut self, Bucket<Self::Pixel>) -> Result<()>; fn close(self) -> Result<()>; }`
  - `unsafe fn shim_open<D: DisplayDriver>(…) -> ndspy_sys::PtDspyError`
  - `unsafe fn shim_data<D: DisplayDriver>(…) -> ndspy_sys::PtDspyError`
  - `unsafe fn shim_close<D: DisplayDriver>(…) -> ndspy_sys::PtDspyError`
  - `unsafe fn shim_query<D: DisplayDriver>(…) -> ndspy_sys::PtDspyError`

- [ ] **Step 1: Write the failing test**

In `shim.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        sync::{Arc, atomic::{AtomicUsize, Ordering}},
    };

    struct Recorder {
        buckets: Arc<AtomicUsize>,
        closed: Arc<AtomicUsize>,
    }

    // A driver's state is normally private; these statics let the test
    // observe it after the shim has taken ownership of the driver.
    static BUCKETS: std::sync::OnceLock<Arc<AtomicUsize>> =
        std::sync::OnceLock::new();
    static CLOSED: std::sync::OnceLock<Arc<AtomicUsize>> =
        std::sync::OnceLock::new();

    impl DisplayDriver for Recorder {
        type Pixel = f32;

        fn open(
            _params: Params<'_>,
            _width: usize,
            _height: usize,
            _format: &PixelFormat,
        ) -> crate::Result<Self> {
            Ok(Recorder {
                buckets: Arc::clone(BUCKETS.get().unwrap()),
                closed: Arc::clone(CLOSED.get().unwrap()),
            })
        }

        fn write(&mut self, bucket: Bucket<'_, f32>) -> crate::Result<()> {
            assert!(bucket.pixels().iter().all(|p| *p == 0.5));
            self.buckets.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn close(self) -> crate::Result<()> {
            self.closed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    /// The full lifecycle through the shims, with the handle reclaimed
    /// exactly once. Miri's leak check is half the assertion.
    #[test]
    fn open_write_close_round_trip() {
        BUCKETS.set(Arc::new(AtomicUsize::new(0))).ok();
        CLOSED.set(Arc::new(AtomicUsize::new(0))).ok();

        let layer = CString::new("beauty.000").unwrap();
        let mut format = [ndspy_sys::PtDspyDevFormat {
            name: layer.as_ptr(),
            type_: 0,
        }];
        let mut flags = ndspy_sys::PtFlagStuff { flags: 0 };
        let filename = CString::new("render").unwrap();
        let drivername = CString::new("recorder").unwrap();
        let mut handle: ndspy_sys::PtDspyImageHandle = core::ptr::null_mut();

        // SAFETY: every pointer below is valid for this call.
        let error = unsafe {
            shim_open::<Recorder>(
                &mut handle,
                drivername.as_ptr(),
                filename.as_ptr(),
                4,
                4,
                0,
                core::ptr::null(),
                format.len() as _,
                format.as_mut_ptr(),
                &mut flags,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert!(!handle.is_null());

        // The driver asked for f32, so the shim must have said so.
        assert_eq!(f32::NDSPY_TYPE, format[0].type_);

        let channels = PixelFormat::from_ndspy(&format).channels();
        let pixels = vec![0.5f32; 2 * 2 * channels];
        // SAFETY: `handle` came from `shim_open`; `pixels` outlives the call.
        let error = unsafe {
            shim_data::<Recorder>(
                handle,
                0,
                2,
                0,
                2,
                core::mem::size_of::<f32>() as _,
                pixels.as_ptr() as *const u8,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(1, BUCKETS.get().unwrap().load(Ordering::SeqCst));

        // SAFETY: `handle` is live and reclaimed exactly here.
        let error = unsafe { shim_close::<Recorder>(handle) };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(1, CLOSED.get().unwrap().load(Ordering::SeqCst));
    }

    /// We must not promise concurrency we do not honour: `write` takes
    /// `&mut self`, so the renderer has to serialise.
    #[test]
    fn we_do_not_advertise_concurrent_buckets() {
        let mut info = ndspy_sys::PtDspyThreadInfo { multithread: 1 };
        // SAFETY: `info` is a valid, correctly sized PtDspyThreadInfo.
        let error = unsafe {
            shim_query::<Recorder>(
                core::ptr::null_mut(),
                ndspy_sys::PtDspyQueryType::Thread,
                core::mem::size_of::<ndspy_sys::PtDspyThreadInfo>() as _,
                &mut info as *mut _ as *mut core::ffi::c_void,
            )
        };
        assert_eq!(ndspy_sys::PtDspyError::None as u32, error as u32);
        assert_eq!(
            0, info.multithread,
            "write takes &mut self, so buckets must be serialised"
        );
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nsi-display shim`
Expected: FAIL to compile — `DisplayDriver` and the shims do not exist.

- [ ] **Step 3: Implement the trait and shims**

`crates/nsi-display/src/shim.rs`:

```rust
//! The trait an author implements, and the shim bodies the macro calls.

use crate::{Bucket, Error, Params, Result};
use core::{
    ffi::{c_char, c_int, c_void},
    panic::AssertUnwindSafe,
};
use nsi_ffi_wrap::output::{PixelFormat, PixelType};

/// A display driver.
///
/// Implement this, then invoke
/// [`declare_display_driver!`](crate::declare_display_driver) to export
/// the symbols the renderer looks for.
///
/// `write` takes `&mut self`, so the renderer is told to serialise
/// buckets ([`PkThreadQuery`] is answered with `multithread = 0`). That
/// is the standard ndspy contract.
pub trait DisplayDriver: Sized + 'static {
    /// The scalar this driver wants its pixels in. The shim rewrites the
    /// renderer's requested format to match.
    type Pixel: PixelType;

    /// Called once, before any pixels.
    fn open(
        params: Params<'_>,
        width: usize,
        height: usize,
        format: &PixelFormat,
    ) -> Result<Self>;

    /// Called once per bucket.
    fn write(&mut self, bucket: Bucket<'_, Self::Pixel>) -> Result<()>;

    /// Called once, after the last bucket. Takes `self`, so the driver
    /// is consumed and cannot be used again.
    fn close(self) -> Result<()>;
}

/// State the shims keep behind `PtDspyImageHandle`.
struct Handle<D: DisplayDriver> {
    driver: D,
    format: PixelFormat,
}

/// Runs `body`, converting a panic into an error code rather than
/// unwinding into C.
fn guard(body: impl FnOnce() -> Error) -> ndspy_sys::PtDspyError {
    match std::panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(error) => error.into(),
        Err(_) => Error::Undefined.into(),
    }
}

/// # Safety
/// Called by the renderer with the ndspy contract's pointers.
#[allow(clippy::too_many_arguments)]
pub unsafe fn shim_open<D: DisplayDriver>(
    image: *mut ndspy_sys::PtDspyImageHandle,
    _drivername: *const c_char,
    _filename: *const c_char,
    width: c_int,
    height: c_int,
    param_count: c_int,
    parameters: *const ndspy_sys::UserParameter,
    format_count: c_int,
    format: *mut ndspy_sys::PtDspyDevFormat,
    flags: *mut ndspy_sys::PtFlagStuff,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() || format.is_null() || format_count <= 0 {
            return Error::BadParameters;
        }

        // SAFETY: the renderer guarantees `format_count` entries.
        let format = unsafe {
            core::slice::from_raw_parts_mut(format, format_count as usize)
        };
        // This driver's scalar is what the renderer must deliver.
        format.iter_mut().for_each(|f| f.type_ = D::Pixel::NDSPY_TYPE);

        let pixel_format = PixelFormat::from_ndspy(format);
        // SAFETY: borrowed for this call only.
        let params = unsafe { Params::from_raw(parameters, param_count) };

        let driver = match D::open(
            params,
            width as usize,
            height as usize,
            &pixel_format,
        ) {
            Ok(driver) => driver,
            Err(error) => return error,
        };

        // The handle is ours: reclaimed in `shim_close`, once.
        let handle = Box::new(Handle { driver, format: pixel_format });
        // SAFETY: `image` and `flags` are valid per the ndspy contract.
        unsafe {
            *image = Box::into_raw(handle) as _;
            if !flags.is_null() {
                (*flags).flags &=
                    !(ndspy_sys::PkDspyFlagsWantsEmptyBuckets as c_int);
            }
        }
        Error::None
    })
}

/// # Safety
/// `image` must have come from `shim_open` and still be open.
pub unsafe fn shim_data<D: DisplayDriver>(
    image: ndspy_sys::PtDspyImageHandle,
    x_min: c_int,
    x_max_plus_one: c_int,
    y_min: c_int,
    y_max_plus_one: c_int,
    _entry_size: c_int,
    data: *const u8,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() {
            return Error::BadParameters;
        }
        if data.is_null() {
            return Error::None;
        }

        // SAFETY: ours, from `shim_open`. `write` needs `&mut`, which is
        // sound because we answer `multithread = 0`.
        let handle = unsafe { &mut *(image as *mut Handle<D>) };

        let channels = handle.format.channels();
        let width = (x_max_plus_one - x_min) as usize;
        let height = (y_max_plus_one - y_min) as usize;

        // SAFETY: the renderer's buffer holds exactly this many values,
        // in the type we requested in `shim_open`.
        let pixels = unsafe {
            core::slice::from_raw_parts(
                data as *const D::Pixel,
                width * height * channels,
            )
        };

        let bucket = Bucket::new(
            x_min as usize,
            x_max_plus_one as usize,
            y_min as usize,
            y_max_plus_one as usize,
            channels,
            pixels,
        );

        match handle.driver.write(bucket) {
            Ok(()) => Error::None,
            Err(error) => error,
        }
    })
}

/// # Safety
/// `image` must have come from `shim_open` and not been closed.
pub unsafe fn shim_close<D: DisplayDriver>(
    image: ndspy_sys::PtDspyImageHandle,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if image.is_null() {
            return Error::BadParameters;
        }
        // SAFETY: ours, reclaimed exactly here. `close` consumes the
        // driver, so the type system prevents a second use.
        let handle = unsafe { Box::from_raw(image as *mut Handle<D>) };
        match handle.driver.close() {
            Ok(()) => Error::None,
            Err(error) => error,
        }
    })
}

/// # Safety
/// `data` must point to `data_len` bytes of the struct the query names.
pub unsafe fn shim_query<D: DisplayDriver>(
    _image: ndspy_sys::PtDspyImageHandle,
    query_type: ndspy_sys::PtDspyQueryType,
    data_len: c_int,
    data: *mut c_void,
) -> ndspy_sys::PtDspyError {
    guard(|| {
        if data.is_null() || data_len <= 0 {
            return Error::BadParameters;
        }
        match query_type {
            ndspy_sys::PtDspyQueryType::Thread => {
                if (data_len as usize)
                    < core::mem::size_of::<ndspy_sys::PtDspyThreadInfo>()
                {
                    return Error::BadParameters;
                }
                // `write` is `&mut self`: the renderer must serialise.
                // SAFETY: length checked above.
                unsafe {
                    (*(data as *mut ndspy_sys::PtDspyThreadInfo)).multithread = 0;
                }
                Error::None
            }
            ndspy_sys::PtDspyQueryType::Progressive => {
                if (data_len as usize)
                    < core::mem::size_of::<ndspy_sys::PtDspyProgressiveInfo>()
                {
                    return Error::BadParameters;
                }
                // SAFETY: length checked above.
                unsafe {
                    (*(data as *mut ndspy_sys::PtDspyProgressiveInfo))
                        .acceptProgressive = 1;
                }
                Error::None
            }
            _ => Error::Unsupported,
        }
    })
}
```

Add to `lib.rs`:

```rust
mod shim;
pub use shim::DisplayDriver;
#[doc(hidden)]
pub use shim::{shim_close, shim_data, shim_open, shim_query};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nsi-display shim`
Expected: PASS, both tests.

- [ ] **Step 5: Run them under Miri**

Run: `cargo +nightly miri test -p nsi-display shim`
Expected: PASS with no leak report — proving the handle is reclaimed
exactly once and nothing borrowed was freed.

- [ ] **Step 6: Commit**

```bash
git add crates/nsi-display/src/shim.rs crates/nsi-display/src/lib.rs
git commit -m "Add the DisplayDriver trait and its shim bodies"
```

---

### Task 6: `declare_display_driver!`

**Files:**

- Modify: `crates/nsi-display/src/lib.rs`
- Test: `crates/nsi-display/tests/symbols.rs` (create)

**Interfaces:**

- Consumes: `shim_open`, `shim_data`, `shim_close`, `shim_query`.
- Produces: macro `declare_display_driver!($driver:ty)` exporting
  `DspyImageOpen`, `DspyImageData`, `DspyImageClose`, `DspyImageQuery`.

- [ ] **Step 1: Write the failing test**

`crates/nsi-display/tests/symbols.rs`:

```rust
//! The macro must export the four symbols the renderer resolves by name.
use nsi_display::{Bucket, DisplayDriver, Params, PixelFormat, Result};

struct Noop;

impl DisplayDriver for Noop {
    type Pixel = f32;
    fn open(_: Params<'_>, _: usize, _: usize, _: &PixelFormat) -> Result<Self> {
        Ok(Noop)
    }
    fn write(&mut self, _: Bucket<'_, f32>) -> Result<()> {
        Ok(())
    }
    fn close(self) -> Result<()> {
        Ok(())
    }
}

nsi_display::declare_display_driver!(Noop);

#[test]
fn the_declared_symbols_are_callable_through_c_signatures() {
    // Taking the symbols at their C types is the assertion: if the macro
    // emitted the wrong signature, this does not compile.
    let open: unsafe extern "C" fn(
        *mut ndspy_sys::PtDspyImageHandle,
        *const core::ffi::c_char,
        *const core::ffi::c_char,
        core::ffi::c_int,
        core::ffi::c_int,
        core::ffi::c_int,
        *const ndspy_sys::UserParameter,
        core::ffi::c_int,
        *mut ndspy_sys::PtDspyDevFormat,
        *mut ndspy_sys::PtFlagStuff,
    ) -> ndspy_sys::PtDspyError = DspyImageOpen;

    let close: unsafe extern "C" fn(
        ndspy_sys::PtDspyImageHandle,
    ) -> ndspy_sys::PtDspyError = DspyImageClose;

    assert!(!(open as usize == 0 || close as usize == 0));
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p nsi-display --test symbols`
Expected: FAIL to compile — `declare_display_driver` does not exist.

- [ ] **Step 3: Implement the macro**

In `lib.rs`:

````rust
/// Exports `$driver` as an ɴsɪ display driver.
///
/// Emits the four symbols the renderer resolves by name. Invoke once,
/// at the crate root of a `cdylib`:
///
/// ```ignore
/// nsi_display::declare_display_driver!(MyDriver);
/// ```
///
/// Build it with `crate-type = ["cdylib"]` and give the artefact the
/// name the renderer expects — 3Delight looks for `<drivername>.dpy`, so
/// `libmy_driver.so` has to be renamed or symlinked to `my_driver.dpy`.
#[macro_export]
macro_rules! declare_display_driver {
    ($driver:ty) => {
        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageOpen(
            image: *mut ::ndspy_sys::PtDspyImageHandle,
            drivername: *const ::core::ffi::c_char,
            filename: *const ::core::ffi::c_char,
            width: ::core::ffi::c_int,
            height: ::core::ffi::c_int,
            param_count: ::core::ffi::c_int,
            parameters: *const ::ndspy_sys::UserParameter,
            format_count: ::core::ffi::c_int,
            format: *mut ::ndspy_sys::PtDspyDevFormat,
            flags: *mut ::ndspy_sys::PtFlagStuff,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe {
                $crate::shim_open::<$driver>(
                    image, drivername, filename, width, height,
                    param_count, parameters, format_count, format, flags,
                )
            }
        }

        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageData(
            image: ::ndspy_sys::PtDspyImageHandle,
            x_min: ::core::ffi::c_int,
            x_max_plus_one: ::core::ffi::c_int,
            y_min: ::core::ffi::c_int,
            y_max_plus_one: ::core::ffi::c_int,
            entry_size: ::core::ffi::c_int,
            data: *const u8,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe {
                $crate::shim_data::<$driver>(
                    image, x_min, x_max_plus_one, y_min, y_max_plus_one,
                    entry_size, data,
                )
            }
        }

        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageClose(
            image: ::ndspy_sys::PtDspyImageHandle,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe { $crate::shim_close::<$driver>(image) }
        }

        /// # Safety
        /// Called by the renderer per the ndspy contract.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn DspyImageQuery(
            image: ::ndspy_sys::PtDspyImageHandle,
            query_type: ::ndspy_sys::PtDspyQueryType,
            data_len: ::core::ffi::c_int,
            data: *mut ::core::ffi::c_void,
        ) -> ::ndspy_sys::PtDspyError {
            unsafe {
                $crate::shim_query::<$driver>(
                    image, query_type, data_len, data,
                )
            }
        }
    };
}
````

Add `ndspy-sys` as a direct dependency of the test target — it is already
a dependency of the crate, and the macro refers to it as `::ndspy_sys`,
so consumers must depend on it too. Document that in the macro's doc
comment.

- [ ] **Step 4: Run the test**

Run: `cargo test -p nsi-display --test symbols`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/nsi-display/src/lib.rs crates/nsi-display/tests/symbols.rs
git commit -m "Add the declare_display_driver! macro"
```

---

### Task 7: Port `r-display`, rendered end to end

Port `crates/ndspy-sys/examples/r-display` onto this crate. Keep its
behaviour -- accumulate buckets, disassociate alpha, write a PNG -- and
drop every unsafe mechanism it needed. Read its `src/lib.rs` first; the
audit table above lists what must not survive the port. The PPM driver
below is a fallback if the `png` dev-dependency proves awkward; prefer
the port.

**Files:**

- Create: `crates/nsi-display/examples/ppm_driver.rs`
- Create: `crates/nsi-display/tests/render.rs`
- Modify: `crates/nsi-display/Cargo.toml`

**Interfaces:**

- Consumes: everything above.
- Produces: nothing further crates depend on.

- [ ] **Step 1: Write the example driver**

`crates/nsi-display/examples/ppm_driver.rs`:

````rust
//! A display driver that writes a binary PPM.
//!
//! Build as a cdylib and give it the name the renderer looks for:
//!
//! ```text
//! cargo build --example ppm_driver
//! cp target/debug/examples/libppm_driver.so ppm.dpy
//! ```
//!
//! Then render with `nsi::string!("drivername", "ppm")`.
use nsi_display::{Bucket, DisplayDriver, Params, PixelFormat, Result};

struct Ppm {
    path: String,
    width: usize,
    height: usize,
    channels: usize,
    pixels: Vec<f32>,
}

impl DisplayDriver for Ppm {
    type Pixel = f32;

    fn open(
        params: Params<'_>,
        width: usize,
        height: usize,
        format: &PixelFormat,
    ) -> Result<Self> {
        let channels = format.channels();
        Ok(Ppm {
            path: params.string("imagefilename").unwrap_or("render").to_owned(),
            width,
            height,
            channels,
            pixels: vec![0.0; width * height * channels],
        })
    }

    fn write(&mut self, bucket: Bucket<'_, f32>) -> Result<()> {
        for y in bucket.y_min()..bucket.y_max() {
            for x in bucket.x_min()..bucket.x_max() {
                let src = ((y - bucket.y_min()) * bucket.width()
                    + (x - bucket.x_min()))
                    * self.channels;
                let dst = (y * self.width + x) * self.channels;
                self.pixels[dst..dst + self.channels]
                    .copy_from_slice(&bucket.pixels()[src..src + self.channels]);
            }
        }
        Ok(())
    }

    fn close(self) -> Result<()> {
        use std::io::Write;
        let mut out = std::io::BufWriter::new(
            std::fs::File::create(format!("{}.ppm", self.path))
                .map_err(|_| nsi_display::Error::NoResource)?,
        );
        write!(out, "P6\n{} {}\n255\n", self.width, self.height)
            .map_err(|_| nsi_display::Error::NoResource)?;
        for pixel in self.pixels.chunks(self.channels) {
            for c in 0..3 {
                let v = pixel.get(c).copied().unwrap_or(0.0);
                out.write_all(&[(v.clamp(0.0, 1.0) * 255.0) as u8])
                    .map_err(|_| nsi_display::Error::NoResource)?;
            }
        }
        Ok(())
    }
}

nsi_display::declare_display_driver!(Ppm);

// A cdylib example still needs a `main` when built as an example target.
fn main() {}
````

Add to `Cargo.toml`:

```toml
[dev-dependencies]
nsi = { version = "0.9", path = "../..", features = ["output"] }

[[example]]
name = "ppm_driver"
crate-type = ["cdylib"]
```

- [ ] **Step 2: Write the failing end-to-end test**

`crates/nsi-display/tests/render.rs`:

```rust
//! Renders through the example driver, proving the symbols are what
//! 3Delight actually resolves.
//!
//! Needs a licensed 3Delight; see AGENTS.md for starting the licence
//! server. Without it the renderer watermarks output but the driver is
//! still exercised, so this test only asserts the driver ran.
use std::{path::Path, process::Command};

#[test]
fn the_example_driver_receives_pixels_from_3delight() {
    // Build the driver and put it where the renderer will find it.
    let status = Command::new(env!("CARGO"))
        .args(["build", "--example", "ppm_driver"])
        .status()
        .expect("cargo build");
    assert!(status.success());

    let built = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug/examples/libppm_driver.so");
    let dpy = std::env::temp_dir().join("ppm.dpy");
    std::fs::copy(&built, &dpy).expect("stage the .dpy");

    // DELIGHT_DISPLAYS is where 3Delight looks for display drivers.
    unsafe {
        std::env::set_var("DELIGHT_DISPLAYS", dpy.parent().unwrap());
    }

    let out = std::env::temp_dir().join("nsi_display_test");
    let ctx = nsi::Context::new(None).expect("context");
    ctx.create("camera", nsi::PERSPECTIVE_CAMERA, None);
    ctx.connect("camera", None, nsi::ROOT, "objects", None);
    ctx.create("screen", nsi::SCREEN, None);
    ctx.connect("screen", None, "camera", "screens", None);
    ctx.set_attribute(
        "screen",
        &[nsi::i32_slice!("resolution", &[16, 16])
            .array_len(const { std::num::NonZeroUsize::new(2).unwrap() })],
    );
    ctx.create("beauty", nsi::OUTPUT_LAYER, None);
    ctx.set_attribute(
        "beauty",
        &[
            nsi::string!("variablename", "Ci"),
            nsi::string!("scalarformat", "float"),
        ],
    );
    ctx.connect("beauty", None, "screen", "outputlayers", None);
    ctx.create("driver", nsi::OUTPUT_DRIVER, None);
    ctx.set_attribute(
        "driver",
        &[
            nsi::string!("drivername", "ppm"),
            nsi::string!("imagefilename", out.to_str().unwrap()),
        ],
    );
    ctx.connect("driver", None, "beauty", "outputdrivers", None);

    ctx.render_control(nsi::Action::Start, None);
    ctx.render_control(nsi::Action::Wait, None);
    drop(ctx);

    let written = out.with_extension("ppm");
    assert!(
        written.exists(),
        "the driver's close() must have written {}",
        written.display()
    );
    let bytes = std::fs::read(&written).expect("read the ppm");
    assert!(bytes.starts_with(b"P6\n16 16\n255\n"), "PPM header");
    assert_eq!(
        b"P6\n16 16\n255\n".len() + 16 * 16 * 3,
        bytes.len(),
        "one RGB triple per pixel"
    );
    let _ = std::fs::remove_file(&written);
}
```

- [ ] **Step 3: Run it and watch it fail**

Run: `cargo test -p nsi-display --test render`
Expected: FAIL — the example does not exist yet, or the `.dpy` is not
found. Fix whichever it reports before proceeding; do not weaken the
assertions to make it pass.

- [ ] **Step 4: Make it pass**

Verify the licence server is running (`$DELIGHT/bin/licutils
serverstatus`), then iterate on `DELIGHT_DISPLAYS` and the artefact name
until the driver is loaded. If 3Delight resolves display drivers by a
different mechanism than `DELIGHT_DISPLAYS`, find the mechanism it does
use and document it in the example's doc comment — that discovery is part
of this task's deliverable, since every user of the crate hits it.

- [ ] **Step 5: Run the test**

Run: `cargo test -p nsi-display --test render`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/nsi-display/examples/ppm_driver.rs \
        crates/nsi-display/tests/render.rs \
        crates/nsi-display/Cargo.toml
git commit -m "Add a worked PPM driver and render it end to end"
```

---

### Task 8: README, crate docs and release readiness

**Files:**

- Create: `crates/nsi-display/README.md`
- Modify: `crates/nsi-display/src/lib.rs`

- [ ] **Step 1: Write the crate-level docs**

At the top of `lib.rs`, a `//!` block covering: what the crate is, the
three-step recipe (implement the trait, invoke the macro, build as
`cdylib`), the artefact-naming requirement discovered in Task 7, and the
four rules the shims enforce. Include the `Ppm` example inline as a

````ignore block.

- [ ] **Step 2: Generate the README from those docs**

Run: `cargo rdme -w nsi-display`
Then: `cargo rdme -w nsi-display --check`
Expected: no diff. The README is generated; never hand-edit it.

- [ ] **Step 3: Verify it is publishable**

Run: `cargo publish --dry-run -p nsi-display --allow-dirty`
Expected: packages cleanly. Check the file list contains only `src/`,
`examples/`, `README.md` and the manifest.

- [ ] **Step 4: Run every gate**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --features output -- -D warnings
cargo test -p nsi-display
cargo +nightly miri test -p nsi-display -- params bucket shim
````

Expected: all clean.

- [ ] **Step 5: Commit**

```bash
git add crates/nsi-display/
git commit -m "Document nsi-display and make it publishable"
```

---

## Self-Review

**Spec coverage.** Every `nsi-display` requirement in the spec maps to a
task: the trait and macro (5, 6), `Params` as a borrowed view (3),
`Bucket` (4), panic containment and handle ownership (5), reuse of
`PixelFormat`/`PixelType` (1, 5), Miri over the shims (5), the worked
example and end-to-end render (7), packaging (8). The spec's
`const MULTITHREAD` is deliberately not implemented — see "Deviation"
above, which is stated rather than silently dropped.

**Placeholders.** None. Task 7 Step 4 asks the implementer to _discover_
how 3Delight resolves driver names, which is a genuine unknown rather than
an unwritten instruction: the plan says what to do with the answer
(document it in the example) and forbids the tempting shortcut (weakening
the assertions).

**Type consistency.** `Params::from_raw`, `Params::string/i32/f32`,
`Bucket::new` with its six arguments, `PixelFormat::from_ndspy`,
`DisplayDriver::{Pixel, open, write, close}` and the four `shim_*`
signatures are used identically everywhere they appear. `Error` is
`nsi_ffi_wrap::output::Error` throughout; `Result<T>` is the crate alias.
