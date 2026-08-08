//! `coreforge-gui`
//!
//! A deliberately basic native GUI for CoreForge - a Git-GUI-style front
//! end, not a polished app. Pick a repository/workspace root, see its
//! modules, click Build/Test/Package/Clean/Workspace Sync, watch the log.
//!
//! Everything that actually resolves or builds a project goes through the
//! `executor` crate, the same one `coreforge-cli` uses - this crate is
//! only a presentation layer on top of it (see [`app::CoreForgeApp`] and
//! [`progress::ChannelProgress`]).

mod app;
mod progress;

use app::CoreForgeApp;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([980.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CoreForge",
        native_options,
        Box::new(|_cc| Ok(Box::new(CoreForgeApp::new()))),
    )
}
