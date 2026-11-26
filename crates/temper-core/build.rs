//! Builds the C++ performance core and links it into the Rust crate.
//!
//! Boundary: C ABI only (`temper_native.h`). C++ stays free of Rust types,
//! Rust owns every buffer passed across the boundary.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=native/temper_native.cpp");
    println!("cargo:rerun-if-changed=native/temper_native.h");

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env != "gnu" && target_env != "msvc" {
        println!("cargo:warning=temper-native: unsupported target env {target_env}");
    }

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++20")
        .include("native")
        .file("native/temper_native.cpp")
        .warnings(true)
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fvisibility=hidden")
        .opt_level(3);

    if target_env == "msvc" {
        build.flag_if_supported("/EHsc");
    } else {
        // Keep the shipped executable free of MinGW runtime DLLs.
        println!("cargo:rustc-link-arg=-static-libgcc");
        println!("cargo:rustc-link-arg=-static-libstdc++");
        println!("cargo:rustc-link-lib=stdc++");
    }

    build.compile("temper_native");
}
