#![cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use bindgen::callbacks::{EnumVariantValue, ParseCallbacks};

use std::{env, path::PathBuf};

#[derive(Debug)]
struct CleanNsiNamingCallbacks {}

impl ParseCallbacks for CleanNsiNamingCallbacks {
    fn item_name(&self, original_item_name: &str) -> Option<String> {
        if original_item_name.starts_with("NSI") {
            Some(original_item_name.trim_end_matches("_t").to_string())
        } else {
            None
        }
    }

    fn enum_variant_name(
        &self,
        enum_name: Option<&str>,
        original_variant_name: &str,
        _variant_value: EnumVariantValue,
    ) -> Option<String> {
        if let Some(enum_name) = enum_name {
            match enum_name {
                "enum NSIErrorLevel" => Some(
                    original_variant_name
                        .trim_start_matches("NSIErr")
                        .to_string(),
                ),
                "enum NSIStoppingStatus" => Some(
                    original_variant_name.trim_start_matches("NSI").to_string(),
                ),
                "enum NSIType_t" => Some(
                    original_variant_name
                        .trim_start_matches("NSIType")
                        .to_string(),
                ),
                _ => None,
            }
        } else {
            None
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=include/wrapper.h");

    let include_path =
        PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("include");

    eprintln!("include: {}", include_path.display());

    // Build bindings
    let mut binding_builder = bindgen::Builder::default()
        .header("include/wrapper.h")
        .allowlist_type("NSI.*")
        .allowlist_type("nsi.*")
        .allowlist_var("NSI.*")
        .rustified_enum("NSI.*")
        .prepend_enum_name(false)
        // Searchpath
        .clang_arg(format!("-I{}", include_path.display()))
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .parse_callbacks(Box::new(CleanNsiNamingCallbacks {}));

    if cfg!(feature = "omit_functions") {
        binding_builder = binding_builder.blocklist_function("NSI.*");
    } else {
        binding_builder = binding_builder.allowlist_function("NSI.*");
    }

    let bindings = binding_builder
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Could not write bindings.");

    Ok(())
}
