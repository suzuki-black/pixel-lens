fn main() {
    tauri_build::build();

    // On macOS: compile the ScreenCaptureKit ObjC shim and link required frameworks.
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/capture_helper.m")
            .flag("-fobjc-arc")
            .flag("-fmodules")
            .compile("capture_helper");

        // ScreenCaptureKit (macOS 12.3+) — SCContentSharingPicker + SCStream
        println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}
