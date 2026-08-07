mod cli;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use camino::Utf8PathBuf;
use clap::Parser;
use cli::{Cli, Command};
use coreforge_core::ModuleId;
use inspector::InspectConfig;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let verbosity = if cli.quiet {
        logging::Verbosity::Quiet
    } else if cli.verbose {
        logging::Verbosity::Verbose
    } else {
        logging::Verbosity::Normal
    };
    logging::init(verbosity);

    tracing::debug!(root = %cli.root, "resolved repository root");
    let build_config = load_build_config(&cli)?;

    match cli.command {
        Command::Build(args) => {
            run_scheduled(&cli.root, &args, &build_config, cli.quiet, "Build", true)?;
        }
        Command::Test(args) => {
            run_scheduled(&cli.root, &args, &build_config, cli.quiet, "Test", false)?;
        }
        Command::Package(args) => {
            let artifacts = run_scheduled(
                &cli.root,
                &args,
                &build_config,
                cli.quiet,
                "Package",
                true,
            )?;
            run_collect(&cli.root, &artifacts)?;
        }
        Command::Clean { module } => run_clean(&cli.root, module.as_deref())?,
        Command::Inspect => run_resolve(&cli.root)?,
        Command::Graph => run_graph(&cli.root)?,
        Command::Workspace(args) => match args.command {
            cli::WorkspaceCommand::Sync => run_workspace_sync(&cli.root)?,
        },
    }

    Ok(())
}

/// Loads `build-system.toml` (Phase 8): `--config` if given, otherwise
/// auto-discovered at `--root`. Returns `None` if no such file exists -
/// that's not an error, every setting simply falls back to its built-in
/// default.
fn load_build_config(cli: &Cli) -> anyhow::Result<Option<config::BuildSystemConfig>> {
    let loaded = match &cli.config {
        Some(path) => Some(config::load(path)?),
        None => config::load_from_root(&cli.root)?,
    };
    if let Some(loaded) = &loaded {
        tracing::debug!(?loaded, "loaded build-system.toml");
    } else {
        tracing::debug!("no build-system.toml found; using built-in defaults");
    }
    Ok(loaded)
}

/// The build settings actually in effect for this invocation, after
/// applying CoreForge's precedence rule: CLI flag > `build-system.toml` >
/// built-in default.
struct EffectiveSettings {
    release: bool,
    /// `0` means "use the number of available CPUs" (passed straight
    /// through to [`scheduler::SchedulerConfig::parallel_jobs`]).
    jobs: usize,
}

fn effective_settings(
    args: &cli::BuildArgs,
    build_config: &Option<config::BuildSystemConfig>,
) -> EffectiveSettings {
    let config_wants_release = build_config
        .as_ref()
        .and_then(|config| config.configuration)
        .is_some_and(|configuration| configuration == config::Configuration::Release);

    let jobs = args
        .jobs
        .or_else(|| build_config.as_ref().and_then(|config| config.parallel_jobs))
        .unwrap_or(0);

    EffectiveSettings {
        release: args.release || config_wants_release,
        jobs,
    }
}

fn print_build_args(args: &cli::BuildArgs, effective: &EffectiveSettings) {
    if args.modules.is_empty() {
        println!("  target: whole module graph");
    } else {
        println!("  target module(s): {}", args.modules.join(", "));
    }
    println!(
        "  configuration: {}",
        if effective.release { "Release" } else { "Debug" }
    );
    println!("  dry-run: {}", args.dry_run);
    println!("  fail-fast: {}", args.fail_fast);
    println!(
        "  parallel jobs: {}",
        if effective.jobs == 0 {
            "auto".to_string()
        } else {
            effective.jobs.to_string()
        }
    );
}

/// Runs the full pipeline over `root` and prints a per-module status
/// report, returning whatever artifacts the toolchain adapters produced
/// (empty when `use_toolchain` is `false`). `Build` and `Package` use the
/// Phase 5 toolchain adapters; `Test`'s adapters aren't implemented yet, so
/// it still uses the scheduler dry-run runner.
fn run_scheduled(
    root: &camino::Utf8Path,
    args: &cli::BuildArgs,
    build_config: &Option<config::BuildSystemConfig>,
    quiet: bool,
    verb: &str,
    use_toolchain: bool,
) -> anyhow::Result<Vec<toolchain::Artifact>> {
    let effective = effective_settings(args, build_config);
    tracing::info!("{verb} command received.");
    print_build_args(args, &effective);

    let inspector_config = InspectConfig::default();
    let project = resolve_project(root, &inspector_config)?;
    let selected_graph = (!args.modules.is_empty())
        .then(|| {
            let targets = args
                .modules
                .iter()
                .map(|module| ModuleId::from(module.as_str()))
                .collect::<Vec<_>>();
            project.graph.dependency_closure(&targets)
        })
        .transpose()?;
    let graph = selected_graph.as_ref().unwrap_or(&project.graph);

    if graph.is_empty() {
        tracing::info!("No modules found under {root}.");
        return Ok(Vec::new());
    }

    if args.dry_run {
        tracing::info!("--dry-run: printing the build plan without running anything.");
        println!("Build order (dependencies first):");
        for (i, id) in graph.build_order()?.iter().enumerate() {
            println!("  {}. {id}", i + 1);
        }
        println!("Parallel build levels:");
        for (level, ids) in graph.build_levels()?.iter().enumerate() {
            let names = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            println!("  level {level}: {names}");
        }
        return Ok(Vec::new());
    }

    let scheduler_config = scheduler::SchedulerConfig {
        parallel_jobs: effective.jobs,
        fail_fast: args.fail_fast,
    };

    let cli_progress = CliProgress::new_if_active(quiet);
    let no_progress = scheduler::NoProgress;
    let progress: &dyn scheduler::ProgressSink = match &cli_progress {
        Some(p) => p,
        None => &no_progress,
    };

    let (report, artifacts) = if use_toolchain {
        let contexts = build_contexts(&project.module_dirs, root, effective.release);
        let runner = toolchain::ToolchainRunner::new(contexts);
        let report = scheduler::run_build_with_progress(graph, &runner, &scheduler_config, progress)?;
        let artifacts = runner.artifacts();
        (report, artifacts)
    } else {
        tracing::info!(
            "{verb} tool adapters are not implemented yet; using the scheduler dry-run runner."
        );
        let report =
            scheduler::run_build_with_progress(graph, &scheduler::DryRunRunner, &scheduler_config, progress)?;
        (report, Vec::new())
    };

    if let Some(cli_progress) = &cli_progress {
        cli_progress.finish();
    } else {
        for outcome in &report.outcomes {
            print_outcome(outcome);
        }
    }

    let succeeded = report
        .outcomes
        .iter()
        .filter(|o| o.status.is_success())
        .count();
    let failed = report.failures().count();
    let skipped = report.skipped().count();
    println!("{verb} finished: {succeeded} succeeded, {failed} failed, {skipped} skipped.");

    if !report.is_success() {
        std::process::exit(1);
    }

    Ok(artifacts)
}

fn print_outcome(outcome: &scheduler::JobOutcome) {
    let (tag, status) = match &outcome.status {
        scheduler::JobStatus::Success => ("OK", logging::Status::Success),
        scheduler::JobStatus::Failed(_) => ("FAIL", logging::Status::Failure),
        scheduler::JobStatus::Skipped(_) => ("SKIP", logging::Status::Warning),
    };
    let detail = match &outcome.status {
        scheduler::JobStatus::Failed(reason) | scheduler::JobStatus::Skipped(reason) => {
            reason.clone()
        }
        scheduler::JobStatus::Success => String::new(),
    };
    let tag = logging::paint(&format!("{tag:<4}"), status);
    println!(
        "  [{tag}] {:<24} {:>7.2?}  {detail}",
        outcome.module, outcome.duration
    );
}

/// A live, per-module progress display driven by [`scheduler::ProgressSink`]
/// events, backed by `indicatif`. Only constructed when stderr is an
/// interactive terminal and `--quiet` was not passed - CI logs and piped
/// output fall back to the plain, static [`print_outcome`] lines instead.
struct CliProgress {
    multi: indicatif::MultiProgress,
    bars: Mutex<HashMap<String, indicatif::ProgressBar>>,
}

impl CliProgress {
    fn new_if_active(quiet: bool) -> Option<Self> {
        if quiet || !logging::is_interactive() {
            return None;
        }
        Some(Self {
            multi: indicatif::MultiProgress::new(),
            bars: Mutex::new(HashMap::new()),
        })
    }

    fn finish(&self) {
        let _ = self.multi.clear();
    }
}

impl scheduler::ProgressSink for CliProgress {
    fn job_started(&self, module: &ModuleId) {
        let bar = self.multi.add(indicatif::ProgressBar::new_spinner());
        bar.set_style(logging::job_progress_style());
        bar.set_message(module.to_string());
        bar.enable_steady_tick(Duration::from_millis(100));
        self.bars
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(module.0.clone(), bar);
    }

    fn job_finished(&self, module: &ModuleId, status: &scheduler::JobStatus, duration: Duration) {
        let existing = self
            .bars
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&module.0);
        // A module skipped before it ever started (blocked by a failed
        // dependency, or fail-fast) never got a bar from `job_started` -
        // give it one now so it still gets a visible line.
        let bar = existing.unwrap_or_else(|| self.multi.add(indicatif::ProgressBar::new_spinner()));

        let (tag, paint_status) = match status {
            scheduler::JobStatus::Success => ("OK", logging::Status::Success),
            scheduler::JobStatus::Failed(_) => ("FAIL", logging::Status::Failure),
            scheduler::JobStatus::Skipped(_) => ("SKIP", logging::Status::Warning),
        };
        let tag = logging::paint(tag, paint_status);
        bar.finish_with_message(format!("[{tag}] {module} ({duration:.2?})"));
    }
}

/// Runs the Project Inspector (Phase 1) + Manifest parser (Phase 2) over
/// `root` and prints the resolved modules, including any `coreforge.toml`
/// overrides and declared dependencies. Note: this only *resolves* modules;
/// nothing is built yet.
fn run_resolve(root: &camino::Utf8Path) -> anyhow::Result<()> {
    tracing::info!("Resolving modules under: {root}");

    let inspector_config = InspectConfig::default();
    let mut modules = if coreforge_workspace::workspace_manifest_exists(root) {
        resolve_graph(root, &inspector_config)?
            .modules()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        manifest::resolve_modules(root, &inspector_config)?
    };
    modules.sort_by(|left, right| left.id.0.cmp(&right.id.0));

    if modules.is_empty() {
        tracing::info!("No modules found under {root}.");
        return Ok(());
    }

    println!("Resolved {} module(s):", modules.len());
    for module in &modules {
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
        println!(
            "  - {:<24} [{:<6}]  root: {:<28} depends: {}",
            module.id, module.module_type, module.root, depends
        );
    }

    Ok(())
}

/// Runs the full pipeline (Inspector + Manifest + Build Graph, Phases 1-3)
/// over `root` and prints the resulting build order and parallel levels.
/// Note: this only *resolves* the graph; nothing is built yet.
fn run_graph(root: &camino::Utf8Path) -> anyhow::Result<()> {
    tracing::info!("Building dependency graph for: {root}");

    let inspector_config = InspectConfig::default();
    let graph = resolve_graph(root, &inspector_config)?;

    if graph.is_empty() {
        tracing::info!("No modules found under {root}.");
        return Ok(());
    }

    println!("{} module(s) in graph.", graph.len());

    println!("Build order (dependencies first):");
    for (i, id) in graph.build_order()?.iter().enumerate() {
        println!("  {}. {id}", i + 1);
    }

    println!("Parallel build levels:");
    for (level, ids) in graph.build_levels()?.iter().enumerate() {
        let names = ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  level {level}: {names}");
    }

    Ok(())
}

fn resolve_graph(
    root: &camino::Utf8Path,
    inspector_config: &InspectConfig,
) -> anyhow::Result<graph::BuildGraph> {
    Ok(resolve_project(root, inspector_config)?.graph)
}

struct ResolvedProject {
    graph: graph::BuildGraph,
    module_dirs: HashMap<ModuleId, Utf8PathBuf>,
}

fn resolve_project(
    root: &camino::Utf8Path,
    inspector_config: &InspectConfig,
) -> anyhow::Result<ResolvedProject> {
    if coreforge_workspace::workspace_manifest_exists(root) {
        let workspace = coreforge_workspace::resolve(root, inspector_config)?;
        let module_dirs = workspace
            .graph
            .modules()
            .map(|module| {
                let location = workspace.module_location(&module.id).ok_or_else(|| {
                    anyhow::anyhow!("workspace module '{}' has no physical location", module.id)
                })?;
                Ok((module.id.clone(), location.module_root.clone()))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;
        Ok(ResolvedProject {
            graph: workspace.graph,
            module_dirs,
        })
    } else {
        let graph = resolver::resolve(root, inspector_config)?;
        let module_dirs = graph
            .modules()
            .map(|module| (module.id.clone(), root.join(&module.root)))
            .collect();
        Ok(ResolvedProject { graph, module_dirs })
    }
}

fn build_contexts(
    module_dirs: &HashMap<ModuleId, Utf8PathBuf>,
    root: &camino::Utf8Path,
    release: bool,
) -> HashMap<ModuleId, toolchain::BuildContext> {
    let profile = if release {
        toolchain::BuildProfile::Release
    } else {
        toolchain::BuildProfile::Debug
    };
    let build_root = root.join(".coreforge").join("build");

    module_dirs
        .iter()
        .map(|(id, module_dir)| {
            (
                id.clone(),
                toolchain::BuildContext {
                    module_dir: module_dir.clone(),
                    output_dir: build_root.join(id.sanitized()),
                    profile,
                },
            )
        })
        .collect()
}

fn run_workspace_sync(root: &camino::Utf8Path) -> anyhow::Result<()> {
    tracing::info!("Synchronizing workspace under: {root}");
    let lock = coreforge_workspace::sync(root)?;
    println!(
        "Workspace lock updated: {} Git repository/repositories pinned.",
        lock.resolved.len()
    );
    for repository in lock.resolved {
        println!("  - {}: {}", repository.name, repository.commit);
    }
    Ok(())
}

/// Runs the Artifact Collector (Phase 6) over whatever the toolchain
/// adapters produced, copying each module's recognizable build output into
/// `root/dist` alongside a `dist-manifest.json`. A module whose managed
/// build directory doesn't contain a recognizable output file is skipped -
/// see [`collector::collect`] - and reported as a warning rather than
/// failing the whole command, since the build itself already succeeded.
fn run_collect(root: &camino::Utf8Path, artifacts: &[toolchain::Artifact]) -> anyhow::Result<()> {
    if artifacts.is_empty() {
        tracing::info!("No artifacts to collect.");
        return Ok(());
    }

    let dist_root = root.join("dist");
    let manifest = collector::collect(artifacts, &dist_root)?;
    collector::write_manifest(&manifest, &dist_root)?;

    let collected = manifest.entries.len();
    println!(
        "Collected {collected}/{} artifact(s) into {dist_root}.",
        artifacts.len()
    );
    for entry in &manifest.entries {
        println!("  - {:<24} {}", entry.module, entry.path);
    }

    let missing = artifacts.len() - collected;
    if missing > 0 {
        tracing::warn!(
            "{missing} module(s) produced a build output directory but no recognizable \
             artifact file was found inside it; nothing was copied for them."
        );
    }

    Ok(())
}

/// Cleans build outputs. Cleaning a single module only removes that
/// module's own managed build directory; cleaning everything (no module
/// given) also removes `dist/`, since it aggregates output across every
/// module and a partial clean would leave it inconsistent.
fn run_clean(root: &camino::Utf8Path, module: Option<&str>) -> anyhow::Result<()> {
    let inspector_config = InspectConfig::default();
    let project = resolve_project(root, &inspector_config)?;
    let contexts = build_contexts(&project.module_dirs, root, false);
    let runner = toolchain::ToolchainRunner::new(contexts);

    let modules = match module {
        Some(module_id) => {
            let id = ModuleId::from(module_id);
            vec![
                project
                    .graph
                    .module(&id)
                    .ok_or_else(|| anyhow::anyhow!("module not found: {module_id}"))?,
            ]
        }
        None => project.graph.modules().collect(),
    };

    for module in modules {
        runner.clean(module)?;
        tracing::info!("Cleaned managed output for module '{}'.", module.id);
    }

    if module.is_none() {
        collector::clean(&root.join("dist"))?;
        tracing::info!("Cleaned dist/ directory.");
    }

    Ok(())
}
