fn main() {
    println!("cargo:rerun-if-env-changed=UMC_JVM_LIB_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    // The embedded backend calls the C API exported by libumc_jvm.so.
    // UMC_JVM_LIB_DIR points at the directory that holds the per-ABI library
    // (the AAR's jni/<abi> directory or an ndk-build obj/local/<abi> tree).
    if let Ok(dir) = std::env::var("UMC_JVM_LIB_DIR") {
        println!("cargo:rustc-link-search=native={dir}");
    }
    println!("cargo:rustc-link-lib=dylib=umc_jvm");
}
