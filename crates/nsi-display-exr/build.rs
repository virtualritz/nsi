//! Bakes OIDN's location into the driver, when it is being built with
//! denoising.
//!
//! The renderer `dlopen`s this driver, so OIDN has to be findable
//! without cargo's environment. An rpath does that; `LD_LIBRARY_PATH`
//! is the wrong tool here, because an OIDN directory may also hold its
//! own TBB, and `mold` loads TBB itself -- putting such a directory on
//! `LD_LIBRARY_PATH` makes the *linker* pick up the wrong one and die
//! on an undefined symbol.
fn main() {
    println!("cargo:rerun-if-env-changed=OIDN_DIR");

    if cfg!(feature = "denoise")
        && let Ok(dir) = std::env::var("OIDN_DIR")
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}/lib");
    }
}
