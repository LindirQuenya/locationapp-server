#!/usr/bin/env bash
export SYSROOT="/mnt/sysroot-pi-staging"
# Some config options so that openssl-sys knows where to find things.
export ARMV7_UNKNOWN_LINUX_GNUEABIHF_OPENSSL_INCLUDE_DIR="$SYSROOT/usr/include/arm-linux-gnueabihf"
export ARMV7_UNKNOWN_LINUX_GNUEABIHF_OPENSSL_LIB_DIR="$SYSROOT/usr/lib/arm-linux-gnueabihf"
export OPENSSL_INCLUDE_DIR="$SYSROOT/usr/include"
export OPENSSL_LIB_DIR="$SYSROOT/usr/lib"
cargo build --target=armv7-unknown-linux-gnueabihf --release
cp target/{armv7-unknown-linux-gnueabihf/,}/release/locationapp-server
