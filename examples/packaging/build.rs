fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icon.ico")
        .set("ProductName", "tgui Packaging")
        .set("CompanyName", "tgui")
        .compile()
        .expect("compile Windows executable icon and version resources");
}
