fn main() {
    tauri_build::build();
    // Library test executables do not inherit Tauri's application resource manifest.
    // TaskDialogIndirect requires Common Controls v6 before the test harness starts.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'");
        // The application already has the complete manifest in Tauri's resource.lib.
        println!("cargo:rustc-link-arg-bin=framefetch=/MANIFEST:NO");
    }
}
