use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let config = cbindgen::Config::from_root_or_default(&crate_dir);

    let output = crate_dir.join("include").join("uvie.h");

    // Parse only the FFI surface. Parsing the whole crate trips a cbindgen bug
    // on the `type OutBuffer = String` aliases in buffers.rs.
    match cbindgen::Builder::new()
        .with_src(crate_dir.join("src").join("ffi.rs"))
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            // write_to_file only rewrites when the contents change, so this
            // does not trigger a rebuild loop.
            bindings.write_to_file(&output);
        }
        Err(e) => {
            // Don't fail the build (e.g. on docs.rs) just because header
            // generation failed; warn instead.
            println!("cargo:warning=cbindgen failed to generate header: {e}");
        }
    }

}
