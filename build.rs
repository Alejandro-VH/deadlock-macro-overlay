fn main() {
    println!("cargo:rerun-if-changed=assets/app.manifest");

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_manifest_file("assets/app.manifest");
        res.set("ProductName", "Deadlock Macro Overlay");
        res.set("FileDescription", "Deadlock Macro Overlay");
        res.set("CompanyName", "Deadlock Macro Team");
        res.set("OriginalFilename", "DeadlockMacroOverlay.exe");
        res.set("LegalCopyright", "MIT License");
        res.compile().expect("Failed to compile Windows PE resources");
    }
}
