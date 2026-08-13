//! Generate the Kotlin + Swift UniFFI bindings for `vautr-ffi` from the
//! compiled library that embeds the uniffi metadata. Used by the mobile
//! native module (`modules/vautr-native`) build (VTR-061).
//!
//! Run: `cargo run -p vautr-ffi --example gen_bindings [out_dir]`
//! (after `cargo build -p vautr-ffi` so the dylib with uniffi metadata exists.)

use camino::Utf8PathBuf;
use uniffi::{generate, GenerateOptions, TargetLanguage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The dylib produced by `cargo build -p vautr-ffi` carries the uniffi
    // component metadata required to generate foreign bindings.
    let lib_path = "target/debug/libvautr_ffi.dylib";
    if !std::path::Path::new(lib_path).exists() {
        eprintln!("error: {lib_path} not found — run `cargo build -p vautr-ffi` first");
        std::process::exit(2);
    }

    let out_dir: Utf8PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "modules/vautr-native/ffi-bindings".to_string())
        .into();
    std::fs::create_dir_all(&out_dir)?;

    let options = GenerateOptions {
        languages: vec![TargetLanguage::Kotlin, TargetLanguage::Swift],
        source: lib_path.into(),
        out_dir: out_dir.clone(),
        config_override: None,
        format: false,
        crate_filter: None,
        metadata_no_deps: true,
    };

    generate(options).map_err(|e| format!("generate_bindings failed: {e:?}"))?;
    println!("generated kotlin + swift bindings -> {out_dir}");
    Ok(())
}
