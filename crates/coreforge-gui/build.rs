//! Embeds `coreforge-emblem.ico` into `coreforge-gui.exe` on Windows, so
//! the window, taskbar, and Explorer all show the CoreForge icon. No-op on
//! every other target - macOS gets its icon via the `.app` bundle built in
//! CI (see `packaging/macos/`), Linux via the `.desktop` entry's `Icon=`.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    winresource::WindowsResource::new()
        .set_icon("../../assets/images/coreforge-emblem.ico")
        .compile()
        .expect("failed to embed the Windows icon resource into coreforge-gui");
}
