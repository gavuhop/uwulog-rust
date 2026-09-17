fn main() {
    println!("cargo:rerun-if-changed=../../packaging/assets/icon.ico");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/assets/icon.ico");
        res.set("FileDescription", "Uwu Log Viewer");
        res.set("ProductName", "Uwu Log");
        res.set("OriginalFilename", "uwu-gui.exe");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource: {}", e);
        }
    }
}
