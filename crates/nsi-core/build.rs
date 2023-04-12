#![cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]

#[cfg(feature = "download_3delight_lib")]
use reqwest;

use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lib_path = PathBuf::from(&env::var("OUT_DIR")?);

    #[cfg(feature = "download_lib3delight")]
    let lib_path = {
        use std::{io::Write, path::Path};

        eprintln!("Building against 3Delight 2.9.30");

        #[cfg(target_os = "windows")]
        let lib = "https://www.dropbox.com/s/9iavkggor0ecc1x/3Delight.dll";
        #[cfg(target_os = "macos")]
        let lib = "https://www.dropbox.com/s/7vle92kcqbbyn8o/lib3delight.dylib";
        #[cfg(target_os = "linux")]
        let lib = "https://www.dropbox.com/s/wfw6w6p41lqd8ko/lib3delight.so";

        let lib_path = lib_path.join(Path::new(lib).file_name().unwrap());

        eprintln!("lib:     {}", lib_path.display());

        if !lib_path.exists() {
            // Download the libs to build against.
            // We do not care of this fails (yet)
            // as this is only needed when the
            // crate is linked against.
            (|| {
                let lib_data = reqwest::blocking::get(lib.to_owned() + "?dl=1")
                    .ok()?
                    .bytes()
                    .ok()?;
                std::fs::File::create(lib_path.clone())
                    .expect(&format!("Could not create {}", lib_path.display()))
                    .write_all(&lib_data)
                    .expect(&format!(
                        "Could not write to {}",
                        lib_path.display()
                    ));
                Some(())
            })();
        }

        lib_path
    };

    eprintln!("lib:     {}", lib_path.display());

    if cfg!(feature = "link_lib3delight") {
        // Emit linker searchpath
        println!(
            "cargo:rustc-link-search={}",
            lib_path.parent().unwrap().display()
        );
        // Link to lib3delight
        println!("cargo:rustc-link-lib=dylib=3delight");
    }

    Ok(())
}
