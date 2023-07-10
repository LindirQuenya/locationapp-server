use std::{env, fs::File, path::Path};

include!("src/config.rs");

/// This function has two tasks: to generate the schema for the config file,
/// and to set the sysroot properly if cross-compiling.
fn main() {
    // We should only regen the schema if config.rs changed.
    // The sysroot thing will never need to be re-run.
    println!("cargo:rerun-if-changed=src/config.rs");

    // Construct the schema file's path. Lifetimes make this a bit tedious.
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let schema_pathbuf = out_dir.join("schema.json");
    let schema_path = schema_pathbuf.as_path();

    // Create the schema file and write the schema as json.
    let schema_file = File::create(schema_path).unwrap();
    let schema = schemars::schema_for!(Config);
    serde_json::to_writer_pretty(schema_file, &schema).expect("Schema deserialization failed!");

    // Note where the schema is, for the user's benefit.
    println!(
        "cargo:warning=Schema at: {}",
        schema_path.as_os_str().to_string_lossy()
    );

    // Figure out what the host and target are.
    let host = env::var("HOST").unwrap();
    let target = env::var("TARGET").unwrap();

    // If we're targeting a raspberry pi but not running on one (cross-compiling),
    // use the sysroot. We read the sysroot path from the environment.
    if target == "armv7-unknown-linux-gnueabihf" && host != target {
        let sysroot = env::var("SYSROOT").expect("SYSROOT must be set for cross-compilation.");
        println!("cargo:rustc-link-arg=--sysroot={}", sysroot);
    }
}
