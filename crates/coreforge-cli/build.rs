//! Embeds `coreforge-emblem.ico` into `coreforge.exe` on Windows, so the
//! binary shows the CoreForge icon in Explorer, the taskbar, and Alt-Tab.
//! No-op on every other target.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    winresource::WindowsResource::new()
        .set_icon("../../assets/images/coreforge-emblem.ico")
        .compile()
        .expect("failed to embed the Windows icon resource into coreforge-cli");
}
