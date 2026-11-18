fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").map(|s| s == "windows").unwrap_or(false) {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("assets/icon.ico");
        r.set("ProductName", "E.D.I.T.H");
        r.set("FileDescription", "E.D.I.T.H");
        if let Err(e) = r.compile() {
            println!("cargo:warning=simge gömülemedi: {e}");
        }
    }
}
