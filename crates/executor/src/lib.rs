//! `executor`
//!
//! Frontend-agnostic build pipeline orchestration, shared by every
//! CoreForge frontend (`coreforge-cli`, and `coreforge-gui`).
//!
//! Nothing in this crate prints or logs anything. Every function returns
//! structured data (or a structured [`ExecutorError`]), and reports live
//! progress only through the [`scheduler::ProgressSink`] the caller
//! supplies. How a result actually gets shown - a terminal with
//! `indicatif` bars, or an `egui` window - is entirely the frontend's job;
//! this crate does not know or care which one is calling it.

mod build;
mod error;
mod project;

pub use build::{
    BuildOptions, BuildOutcome, DryRunPlan, EffectiveSettings, build, clean, dry_run,
    effective_settings, inspect, package, resolve_graph, test, workspace_sync,
};
pub use error::{ExecutorError, Result};
pub use project::{Project, build_contexts, resolve_project, select_graph};
