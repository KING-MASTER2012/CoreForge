//! High-level build pipeline operations shared by every CoreForge
//! frontend: resolve a project, decide the effective settings, run the
//! scheduler, optionally collect artifacts. Nothing here prints or logs -
//! every function returns structured data; live progress only goes through
//! the [`scheduler::ProgressSink`] the caller supplies.

use camino::Utf8Path;
use coreforge_core::{Module, ModuleId};

use crate::error::{ExecutorError, Result};
use crate::project::{build_contexts, resolve_project, select_graph};

/// What a caller is asking for: which modules (empty = every module in the
/// resolved graph, plus their dependencies), and the knobs that map onto
/// [`scheduler::SchedulerConfig`] / [`toolchain::BuildProfile`].
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Module ids to build (their dependencies are pulled in
    /// automatically). Empty means every module in the resolved graph.
    pub modules: Vec<String>,
    /// Forces the `Release` profile. There is no explicit "force Debug"
    /// counterpart - leaving this `false` just means "don't override";
    /// see [`effective_settings`] for how it combines with
    /// `build-system.toml`.
    pub release: bool,
    /// Maximum modules to build in parallel. `None` falls back to
    /// `build-system.toml`, then to the number of available CPUs.
    pub jobs: Option<usize>,
    /// Stop scheduling new modules once one has failed.
    pub fail_fast: bool,
}

/// The build settings actually in effect once [`BuildOptions`] and an
/// optional loaded `build-system.toml` are combined, following
/// CoreForge's precedence rule: explicit option > `build-system.toml` >
/// built-in default.
#[derive(Debug, Clone, Copy)]
pub struct EffectiveSettings {
    /// Whether to build in the `Release` profile.
    pub release: bool,
    /// Maximum modules to build in parallel. `0` means "use the number of
    /// available CPUs" - passed straight through to
    /// [`scheduler::SchedulerConfig::parallel_jobs`].
    pub jobs: usize,
}

/// Combines `options` with an optional loaded `build-system.toml`.
#[must_use]
pub fn effective_settings(
    options: &BuildOptions,
    build_config: Option<&config::BuildSystemConfig>,
) -> EffectiveSettings {
    let config_wants_release = build_config
        .and_then(|config| config.configuration)
        .is_some_and(|configuration| configuration == config::Configuration::Release);

    let jobs = options
        .jobs
        .or_else(|| build_config.and_then(|config| config.parallel_jobs))
        .unwrap_or(0);

    EffectiveSettings {
        release: options.release || config_wants_release,
        jobs,
    }
}

/// The build order and parallel levels for `options.modules` (and their
/// dependencies) under `root`, without building anything.
#[derive(Debug, Clone)]
pub struct DryRunPlan {
    /// Full build order, dependencies first.
    pub order: Vec<ModuleId>,
    /// The same modules grouped into levels that can run in parallel.
    pub levels: Vec<Vec<ModuleId>>,
}

/// The result of a [`build`] or [`package`] run: the scheduler's
/// per-module report, plus whatever artifacts the toolchain adapters
/// produced (empty for module types without a toolchain adapter, e.g.
/// SQL).
#[derive(Debug)]
pub struct BuildOutcome {
    /// Per-module outcomes (success, failure, or skipped) and timing.
    pub report: scheduler::SchedulerReport,
    /// Artifacts produced by successful jobs.
    pub artifacts: Vec<toolchain::Artifact>,
}

/// Resolves `root` and lists every module found, sorted by id. Builds
/// nothing.
///
/// # Errors
///
/// See [`crate::ExecutorError`].
pub fn inspect(root: &Utf8Path) -> Result<Vec<Module>> {
    let project = resolve_project(root)?;
    let mut modules = project.graph.modules().cloned().collect::<Vec<_>>();
    modules.sort_by(|left, right| left.id.0.cmp(&right.id.0));
    Ok(modules)
}

/// Resolves `root` into its dependency graph. Builds nothing.
///
/// # Errors
///
/// See [`crate::ExecutorError`].
pub fn resolve_graph(root: &Utf8Path) -> Result<graph::BuildGraph> {
    Ok(resolve_project(root)?.graph)
}

/// Computes the build order and parallel levels for `options.modules`
/// under `root`, without building anything.
///
/// # Errors
///
/// See [`crate::ExecutorError`].
pub fn dry_run(root: &Utf8Path, options: &BuildOptions) -> Result<DryRunPlan> {
    let project = resolve_project(root)?;
    let selected = select_graph(&project, &options.modules)?;
    let graph = selected.as_ref().unwrap_or(&project.graph);

    Ok(DryRunPlan {
        order: graph.build_order()?,
        levels: graph.build_levels()?,
    })
}

/// Builds `options.modules` (and their dependencies) under `root` using
/// the Phase 5 toolchain adapters (Cargo, CMake+Ninja, Go), reporting
/// progress to `progress` as jobs start and finish.
///
/// # Errors
///
/// See [`crate::ExecutorError`]. A failed *module* build is not itself an
/// `Err` here - it shows up as a [`scheduler::JobStatus::Failed`] entry in
/// the returned report; callers decide what "the command failed" means
/// (the CLI treats any non-success report as a nonzero exit).
pub fn build(
    root: &Utf8Path,
    options: &BuildOptions,
    build_config: Option<&config::BuildSystemConfig>,
    progress: &dyn scheduler::ProgressSink,
) -> Result<BuildOutcome> {
    let effective = effective_settings(options, build_config);
    let project = resolve_project(root)?;
    let selected = select_graph(&project, &options.modules)?;
    let graph = selected.as_ref().unwrap_or(&project.graph);

    let scheduler_config = scheduler::SchedulerConfig {
        parallel_jobs: effective.jobs,
        fail_fast: options.fail_fast,
    };
    let contexts = build_contexts(&project.module_dirs, root, effective.release);
    let runner = toolchain::ToolchainRunner::new(contexts);

    let report = scheduler::run_build_with_progress(graph, &runner, &scheduler_config, progress)?;
    let artifacts = runner.artifacts();
    Ok(BuildOutcome { report, artifacts })
}

/// Runs `options.modules` (and their dependencies) through the
/// scheduler's dry-run runner instead of a real toolchain adapter.
///
/// `coreforge test` doesn't have a real test adapter yet - no toolchain
/// crate integration for `cargo test` / `ctest` / `go test` exists. This
/// exists so the command still reports a build order and per-module
/// timing (all trivially successful) instead of doing nothing at all.
/// Replace this with a real, adapter-backed implementation once the
/// toolchain crate grows test support.
///
/// # Errors
///
/// See [`crate::ExecutorError`].
pub fn test(
    root: &Utf8Path,
    options: &BuildOptions,
    progress: &dyn scheduler::ProgressSink,
) -> Result<BuildOutcome> {
    let effective = effective_settings(options, None);
    let project = resolve_project(root)?;
    let selected = select_graph(&project, &options.modules)?;
    let graph = selected.as_ref().unwrap_or(&project.graph);

    let scheduler_config = scheduler::SchedulerConfig {
        parallel_jobs: effective.jobs,
        fail_fast: options.fail_fast,
    };

    let report = scheduler::run_build_with_progress(
        graph,
        &scheduler::DryRunRunner,
        &scheduler_config,
        progress,
    )?;
    Ok(BuildOutcome {
        report,
        artifacts: Vec::new(),
    })
}

/// Runs [`build`], then collects every produced artifact into `root/dist`
/// via the Artifact Collector (Phase 6).
///
/// # Errors
///
/// See [`crate::ExecutorError`].
pub fn package(
    root: &Utf8Path,
    options: &BuildOptions,
    build_config: Option<&config::BuildSystemConfig>,
    progress: &dyn scheduler::ProgressSink,
) -> Result<(BuildOutcome, collector::DistManifest)> {
    let outcome = build(root, options, build_config, progress)?;
    let dist_root = root.join("dist");
    let manifest = collector::collect(&outcome.artifacts, &dist_root)?;
    collector::write_manifest(&manifest, &dist_root)?;
    Ok((outcome, manifest))
}

/// Cleans build output and returns the ids of every module that was
/// cleaned.
///
/// `module` given: only that module's managed build directory. `module`
/// omitted: every module's build directory, plus `dist/` (which
/// aggregates across all modules, so a partial clean would leave it
/// inconsistent).
///
/// # Errors
///
/// Returns [`ExecutorError::ModuleNotFound`] if `module` doesn't exist in
/// the resolved graph. See [`crate::ExecutorError`] for other cases.
pub fn clean(root: &Utf8Path, module: Option<&str>) -> Result<Vec<ModuleId>> {
    let project = resolve_project(root)?;
    let contexts = build_contexts(&project.module_dirs, root, false);
    let runner = toolchain::ToolchainRunner::new(contexts);

    let modules: Vec<&Module> = match module {
        Some(module_id) => {
            let id = ModuleId::from(module_id);
            vec![
                project
                    .graph
                    .module(&id)
                    .ok_or_else(|| ExecutorError::ModuleNotFound(module_id.to_string()))?,
            ]
        }
        None => project.graph.modules().collect(),
    };

    let mut cleaned = Vec::with_capacity(modules.len());
    for module in modules {
        runner.clean(module)?;
        cleaned.push(module.id.clone());
    }

    if module.is_none() {
        collector::clean(&root.join("dist"))?;
    }

    Ok(cleaned)
}

/// Synchronizes a workspace's Git repositories: clones or updates each
/// repository declared in `coreforge-workspace.toml` and pins their
/// resolved commits into `coreforge-workspace.lock`.
///
/// # Errors
///
/// See [`crate::ExecutorError`].
pub fn workspace_sync(root: &Utf8Path) -> Result<coreforge_workspace::WorkspaceLockFile> {
    Ok(coreforge_workspace::sync(root)?)
}
