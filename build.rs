use schemars;
use serde_json;
use std::{env, fs::File, path::Path};

include!("src/config.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/config.rs");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let schema = schemars::schema_for!(Config);
    let schema_pathbuf = out_dir.join("schema.json");
    let schema_path = schema_pathbuf.as_path();
    let schema_file = File::create(schema_path).unwrap();
    serde_json::to_writer_pretty(schema_file, &schema).expect("Schema deserialization failed!");
    println!(
        "cargo:warning=Schema at: {}",
        schema_path.as_os_str().to_string_lossy()
    );
    let host = env::var("HOST").unwrap();
    let target = env::var("TARGET").unwrap();
    // Check: is the host (builder) a non-raspberry-pi?
    if target == "armv7-unknown-linux-gnueabihf" && host != target {
        let sysroot = env::var("SYSROOT").expect("SYSROOT must be set for cross-compilation.");
        println!("cargo:rustc-link-arg=--sysroot={}", sysroot);
    }
}
