use std::env;

fn main() {
    let host = env::var("HOST").unwrap();
    let target = env::var("TARGET").unwrap();
    // Check: is the host (builder) a raspberry pi?
    if target == "armv7-unknown-linux-gnueabihf" && host != target {
        let sysroot = env::var("SYSROOT").expect("SYSROOT must be set for cross-compilation.");
        println!("cargo:rustc-link-arg=--sysroot={}", sysroot);
    }
}
