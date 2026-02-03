//! Generate UniFFI Swift bindings for AWB-RS
//!
//! Run with: cargo run -p gen_swift_bindings

use camino::Utf8PathBuf;
use uniffi_bindgen::bindings::SwiftBindingsOptions;

fn main() -> anyhow::Result<()> {
    let udl_file = Utf8PathBuf::from("crates/awb_ffi/src/awb_ffi.udl");
    let out_dir = Utf8PathBuf::from("ui/macos/AWBrowser/Sources/AWBrowser/Generated");

    // Create output directory
    std::fs::create_dir_all(&out_dir)?;

    println!("Generating Swift bindings from {} to {}", udl_file, out_dir);

    let options = SwiftBindingsOptions {
        generate_swift_sources: true,
        generate_headers: true,
        generate_modulemap: true,
        source: udl_file,
        out_dir: out_dir.clone(),
        xcframework: false,
        module_name: Some("AwbFfi".to_string()),
        modulemap_filename: None,
        metadata_no_deps: false,
        link_frameworks: vec![],
    };

    uniffi_bindgen::bindings::generate_swift_bindings(options)?;

    println!("✓ Swift bindings generated successfully in {}", out_dir);
    Ok(())
}
