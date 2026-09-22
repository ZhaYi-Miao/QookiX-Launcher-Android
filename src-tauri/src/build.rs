fn main() {
    tauri_build::try_build(tauri_build::Attributes::new()).expect("failed to run tauri build");

    println!("cargo:rerun-if-changed=src");

    let out_dir = std::env::var("OUT_DIR").unwrap_or_else(|_| {
        let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
        format!("{}/out", target)
    });

    std::fs::create_dir_all(&out_dir).unwrap_or_default();

    println!("cargo:rustc-env=OUT_DIR={}", out_dir);

    // Android: link GLES library (EGL is dynamically loaded via dlopen)
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        println!("cargo:rustc-link-lib=GLESv3");
    }
}
