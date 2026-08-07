//! `logging`
//!
//! Diagnostics UI (Phase 7).
//!
//! Every other crate reports what it's doing with plain `tracing`
//! (`tracing::info!`, `tracing::warn!`, ...) - this crate is the one place
//! that decides how those events actually get rendered: colored or plain,
//! filtered by `RUST_LOG` or by `--verbose`/`--quiet`, and whether stderr is
//! an interactive terminal at all (CI and piped output get static text, not
//! ANSI escapes or a redrawing progress bar).
//!
//! `coreforge-cli` calls [`init`] exactly once, as early as possible in
//! `main`. No other crate depends on `logging` - keeping the dependency
//! one-directional means `scheduler`, `toolchain`, etc. can emit `tracing`
//! events without knowing (or caring) how the CLI chooses to display them.

use std::io::IsTerminal;

use owo_colors::OwoColorize;
use tracing_subscriber::EnvFilter;

/// How much detail CoreForge should report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    /// Only warnings and errors (`-q`/`--quiet`).
    Quiet,
    /// The default: informational progress, plus warnings and errors.
    Normal,
    /// Everything, including internal debug detail (`-v`/`--verbose`).
    Verbose,
}

impl Verbosity {
    const fn default_filter(self) -> &'static str {
        match self {
            Self::Quiet => "warn",
            Self::Normal => "info",
            Self::Verbose => "debug",
        }
    }
}

/// Initializes the global `tracing` subscriber for the CoreForge CLI.
///
/// `RUST_LOG` always wins if set (letting a developer filter per-crate,
/// e.g. `RUST_LOG=toolchain=debug`); otherwise the filter is derived from
/// `verbosity`. Safe to call more than once (e.g. across tests) - later
/// calls are silently ignored rather than panicking.
pub fn init(verbosity: Verbosity) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(verbosity.default_filter()));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(is_interactive())
        .without_time()
        .try_init();
}

/// Whether stderr is an interactive terminal.
///
/// CoreForge only draws progress bars and colors output when this is true;
/// CI logs and piped output get plain, static, uncolored text instead.
#[must_use]
pub fn is_interactive() -> bool {
    std::io::stderr().is_terminal()
}

/// The semantic color a piece of status text should carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// A successful outcome (green).
    Success,
    /// A failed outcome (red).
    Failure,
    /// A skipped or warning outcome (yellow).
    Warning,
}

/// Colors `text` for `status`, unless stderr is not an interactive terminal
/// - in which case `text` is returned unchanged, since raw ANSI escapes in
/// a CI log or a piped file just add noise.
#[must_use]
pub fn paint(text: &str, status: Status) -> String {
    if !is_interactive() {
        return text.to_string();
    }
    match status {
        Status::Success => text.green().to_string(),
        Status::Failure => text.red().to_string(),
        Status::Warning => text.yellow().to_string(),
    }
}

/// The [`indicatif::ProgressStyle`] used for one module's live status line
/// while it builds. Centralized here so every caller gets the same look
/// instead of each hand-rolling its own template string.
#[must_use]
pub fn job_progress_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .unwrap_or_else(|_| indicatif::ProgressStyle::default_spinner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_is_stricter_than_verbose() {
        assert_eq!(Verbosity::Quiet.default_filter(), "warn");
        assert_eq!(Verbosity::Verbose.default_filter(), "debug");
    }

    #[test]
    fn init_does_not_panic_when_called_twice() {
        init(Verbosity::Normal);
        init(Verbosity::Verbose);
    }
}
