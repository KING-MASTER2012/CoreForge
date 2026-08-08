//! The application: state, the `eframe::App::update` loop, and the
//! background-thread helpers that keep the UI responsive while an
//! `executor::` pipeline runs.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

use camino::Utf8PathBuf;
use coreforge_core::Module;

use crate::progress::{ChannelProgress, GuiEvent, JobOutcomeKind, summarize};

/// Which `executor` entry point [`CoreForgeApp::run`] should call.
#[derive(Debug, Clone, Copy)]
enum RunMode {
    Build,
    /// `coreforge test` doesn't have a real test adapter yet - see
    /// [`executor::test`].
    Test,
    Package,
}

impl RunMode {
    const fn verb(self) -> &'static str {
        match self {
            Self::Build => "Build",
            Self::Test => "Test",
            Self::Package => "Package",
        }
    }
}

/// The semantic color a log line should carry.
#[derive(Clone, Copy)]
enum LogKind {
    Info,
    Success,
    Warning,
    Error,
}

impl LogKind {
    fn color(self) -> egui::Color32 {
        match self {
            Self::Info => egui::Color32::from_gray(190),
            Self::Success => egui::Color32::from_rgb(90, 200, 120),
            Self::Warning => egui::Color32::from_rgb(230, 180, 60),
            Self::Error => egui::Color32::from_rgb(235, 100, 100),
        }
    }
}

struct LogLine {
    text: String,
    kind: LogKind,
}

/// The whole application's state. A single window, no tabs, no theming -
/// deliberately as basic as a GUI gets while still doing the job.
pub struct CoreForgeApp {
    root: Option<Utf8PathBuf>,
    modules: Vec<Module>,
    build_config: Option<config::BuildSystemConfig>,

    release: bool,
    fail_fast: bool,
    dry_run: bool,
    jobs: usize,
    module_filter: String,

    busy: bool,
    log: Vec<LogLine>,

    tx: Sender<GuiEvent>,
    rx: Receiver<GuiEvent>,
}

impl CoreForgeApp {
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            root: None,
            modules: Vec::new(),
            build_config: None,
            release: false,
            fail_fast: false,
            dry_run: false,
            jobs: 0,
            module_filter: String::new(),
            busy: false,
            log: Vec::new(),
            tx,
            rx,
        }
    }

    fn push_log(&mut self, text: impl Into<String>, kind: LogKind) {
        self.log.push(LogLine {
            text: text.into(),
            kind,
        });
    }

    /// Turns the current form state into an [`executor::BuildOptions`].
    /// The module filter is a comma-separated list; an empty field means
    /// "every module".
    fn build_options(&self) -> executor::BuildOptions {
        let modules = self
            .module_filter
            .split(',')
            .map(str::trim)
            .filter(|module| !module.is_empty())
            .map(str::to_string)
            .collect();
        executor::BuildOptions {
            modules,
            release: self.release,
            jobs: if self.jobs == 0 {
                None
            } else {
                Some(self.jobs)
            },
            fail_fast: self.fail_fast,
        }
    }

    /// The module filter interpreted as a single `Clean` target: `None`
    /// (clean everything) unless the field names exactly one module.
    fn clean_target(&self) -> Option<String> {
        let trimmed = self.module_filter.trim();
        if trimmed.is_empty() || trimmed.contains(',') {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn pick_root(&mut self) {
        let Some(path) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        match Utf8PathBuf::from_path_buf(path) {
            Ok(root) => {
                self.push_log(format!("Root directory selected: {root}"), LogKind::Info);
                self.root = Some(root);
                self.refresh();
            }
            Err(_) => self.push_log("Selected folder path is not valid UTF-8.", LogKind::Error),
        }
    }

    /// Re-resolves the module list and reloads `build-system.toml` for the
    /// current root. Runs on the UI thread - `inspect`/`load_from_root`
    /// are expected to be fast (no network, no compilation).
    fn refresh(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };

        match executor::inspect(&root) {
            Ok(modules) => {
                self.push_log(format!("{} modules found.", modules.len()), LogKind::Info);
                self.modules = modules;
            }
            Err(error) => {
                self.modules.clear();
                self.push_log(format!("Module resolution error: {error}"), LogKind::Error);
            }
        }

        match config::load_from_root(&root) {
            Ok(build_config) => self.build_config = build_config,
            Err(error) => {
                self.build_config = None;
                self.push_log(
                    format!("Failed to read build-system.toml: {error}"),
                    LogKind::Warning,
                );
            }
        }
    }

    fn run(&mut self, mode: RunMode) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let options = self.build_options();

        if self.dry_run {
            self.print_dry_run(&root, &options);
            return;
        }

        self.busy = true;
        self.push_log(format!("{} started.", mode.verb()), LogKind::Info);

        let build_config = self.build_config.clone();
        let sender = self.tx.clone();
        thread::spawn(move || {
            let progress = ChannelProgress::new(sender.clone());
            let result = match mode {
                RunMode::Build => {
                    executor::build(&root, &options, build_config.as_ref(), &progress)
                        .map(|outcome| summarize("Build", &outcome, None))
                }
                RunMode::Test => executor::test(&root, &options, &progress)
                    .map(|outcome| summarize("Test", &outcome, None)),
                RunMode::Package => {
                    executor::package(&root, &options, build_config.as_ref(), &progress).map(
                        |(outcome, manifest)| {
                            summarize("Package", &outcome, Some(manifest.entries.len()))
                        },
                    )
                }
            };
            let _ = sender.send(GuiEvent::RunFinished(
                result.map_err(|error| error.to_string()),
            ));
        });
    }

    fn print_dry_run(&mut self, root: &Utf8PathBuf, options: &executor::BuildOptions) {
        match executor::dry_run(root, options) {
            Ok(plan) if plan.order.is_empty() => {
                self.push_log("Build plan is empty: no modules found.", LogKind::Warning);
            }
            Ok(plan) => {
                self.push_log("Build order (dependencies first):", LogKind::Info);
                for (i, id) in plan.order.iter().enumerate() {
                    self.push_log(format!("  {}. {id}", i + 1), LogKind::Info);
                }
                for (level, ids) in plan.levels.iter().enumerate() {
                    let names = ids
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.push_log(format!("  level {level}: {names}"), LogKind::Info);
                }
            }
            Err(error) => self.push_log(format!("Error: {error}"), LogKind::Error),
        }
    }

    fn clean(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let target = self.clean_target();

        self.busy = true;
        self.push_log("Clean started.", LogKind::Info);

        let sender = self.tx.clone();
        thread::spawn(move || {
            let result = executor::clean(&root, target.as_deref());
            let _ = sender.send(GuiEvent::CleanFinished(
                result
                    .map(|cleaned| cleaned.len())
                    .map_err(|error| error.to_string()),
            ));
        });
    }

    fn workspace_sync(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };

        self.busy = true;
        self.push_log("Workspace sync started.", LogKind::Info);

        let sender = self.tx.clone();
        thread::spawn(move || {
            let result = executor::workspace_sync(&root);
            let _ = sender.send(GuiEvent::WorkspaceSyncFinished(
                result
                    .map(|lock| lock.resolved.len())
                    .map_err(|error| error.to_string()),
            ));
        });
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                GuiEvent::JobStarted(id) => self.push_log(format!("started: {id}"), LogKind::Info),
                GuiEvent::JobFinished(id, kind, duration) => {
                    let (tag, log_kind) = match &kind {
                        JobOutcomeKind::Success => ("OK", LogKind::Success),
                        JobOutcomeKind::Failed(_) => ("FAIL", LogKind::Error),
                        JobOutcomeKind::Skipped(_) => ("SKIP", LogKind::Warning),
                    };
                    let detail = match &kind {
                        JobOutcomeKind::Failed(reason) | JobOutcomeKind::Skipped(reason) => {
                            format!(" - {reason}")
                        }
                        JobOutcomeKind::Success => String::new(),
                    };
                    self.push_log(format!("[{tag}] {id} ({duration:.2?}){detail}"), log_kind);
                }
                GuiEvent::RunFinished(result) => {
                    self.busy = false;
                    match result {
                        Ok(summary) => self.push_log(summary.to_string(), LogKind::Info),
                        Err(error) => self.push_log(format!("Error: {error}"), LogKind::Error),
                    }
                }
                GuiEvent::CleanFinished(result) => {
                    self.busy = false;
                    match result {
                        Ok(count) => self.push_log(
                            format!("Clean completed: {count} modules cleaned."),
                            LogKind::Success,
                        ),
                        Err(error) => self.push_log(format!("Error: {error}"), LogKind::Error),
                    }
                }
                GuiEvent::WorkspaceSyncFinished(result) => {
                    self.busy = false;
                    match result {
                        Ok(count) => self.push_log(
                            format!("Workspace sync completed: {count} repos pinned."),
                            LogKind::Success,
                        ),
                        Err(error) => self.push_log(format!("Error: {error}"), LogKind::Error),
                    }
                }
            }
        }
    }
}

impl Default for CoreForgeApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for CoreForgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        if self.busy {
            // Keep repainting while a background thread is working so log
            // lines and the busy spinner actually animate.
            ui.ctx().request_repaint_after(Duration::from_millis(120));
        }

        // Top Bar
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Select Folder..."))
                .clicked()
            {
                self.pick_root();
            }
            match &self.root {
                Some(root) => {
                    ui.label(root.as_str());
                }
                None => {
                    ui.weak("(no folder selected)");
                }
            }
            if ui
                .add_enabled(
                    !self.busy && self.root.is_some(),
                    egui::Button::new("Refresh"),
                )
                .clicked()
            {
                self.refresh();
            }
        });
        ui.add_space(4.0);
        ui.separator();

        // Main layout split
        ui.horizontal(|ui| {
            // Left Column (Modules)
            ui.vertical(|ui| {
                ui.set_width(300.0);
                ui.heading("Modules");
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("modules_scroll")
                    .show(ui, |ui| {
                        if self.modules.is_empty() {
                            ui.weak("No modules.");
                        }
                        for module in &self.modules {
                            let depends = if module.depends.is_empty() {
                                String::from("-")
                            } else {
                                module
                                    .depends
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            };
                            ui.label(format!("{}  [{}]", module.id, module.module_type));
                            ui.weak(format!("  depends: {depends}"));
                        }
                    });
            });

            ui.separator();

            // Right Column (Controls & Logs)
            ui.vertical(|ui| {
                // Controls at the top of the right section
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.release, "Release");
                        ui.checkbox(&mut self.fail_fast, "Fail-fast");
                        ui.checkbox(&mut self.dry_run, "Dry-run");
                        ui.separator();
                        ui.label("Jobs:");
                        ui.add(egui::DragValue::new(&mut self.jobs).range(0..=256))
                            .on_hover_text("0 = automatic (CPU count)");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Target module(s):");
                        ui.text_edit_singleline(&mut self.module_filter)
                            .on_hover_text(
                                "Comma-separated module IDs. Empty means whole graph. \
                             For Clean: a single name or empty.",
                            );
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let has_root = self.root.is_some();
                        if ui
                            .add_enabled(has_root, egui::Button::new("Build"))
                            .clicked()
                        {
                            self.run(RunMode::Build);
                        }
                        if ui
                            .add_enabled(has_root, egui::Button::new("Test"))
                            .clicked()
                        {
                            self.run(RunMode::Test);
                        }
                        if ui
                            .add_enabled(has_root, egui::Button::new("Package"))
                            .clicked()
                        {
                            self.run(RunMode::Package);
                        }
                        ui.separator();
                        if ui
                            .add_enabled(has_root, egui::Button::new("Clean"))
                            .clicked()
                        {
                            self.clean();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(has_root, egui::Button::new("Workspace Sync"))
                            .clicked()
                        {
                            self.workspace_sync();
                        }
                    });
                    if self.busy {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Running...");
                        });
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.heading("Logs");
                ui.separator();

                // Logs filling the rest of the right column
                egui::ScrollArea::vertical()
                    .id_salt("log_scroll")
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.log {
                            ui.colored_label(line.kind.color(), line.text.as_str());
                        }
                    });
            });
        });
    }
}
