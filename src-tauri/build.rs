fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    panda_onnx();
    link_compiler_rt();
    tauri_build::build()
}

/// Targets where the bundled Panda Router ONNX profiler is linkable.
fn panda_onnx() {
    println!("cargo:rustc-check-cfg=cfg(panda_onnx)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if matches!((os.as_str(), arch.as_str()), ("macos", "aarch64") | ("linux", "aarch64") | ("windows", _)) {
        println!("cargo:rustc-cfg=panda_onnx");
    }
}

/// ONNX Runtime CoreML objects use clang compiler-rt availability helpers on Apple Silicon.
fn link_compiler_rt() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64") { return; }
    let Ok(output) = std::process::Command::new("xcrun").args(["clang", "--print-resource-dir"]).output() else { return; };
    let dir = std::path::PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()).join("lib/darwin");
    if dir.join("libclang_rt.osx.a").is_file() {
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=static=clang_rt.osx");
    }
}
