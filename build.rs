fn main() {
    #[cfg(target_os = "windows")]
    if std::env::var("CARGO_FEATURE_VIDEO").is_ok() {
        println!("cargo:rustc-link-lib=strmiids");
        println!("cargo:rustc-link-lib=mfuuid");
    }
}
