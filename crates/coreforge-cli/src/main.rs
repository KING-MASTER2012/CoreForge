mod cli;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use clap::Parser;
use cli::{Cli, Command};
use coreforge_core::ModuleId;

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
            run_build_like(&cli.root, &args, &build_config, cli.quiet, RunMode::Build)?;
        }
        Command::Test(args) => {
            run_build_like(&cli.root, &args, &build_config, cli.quiet, RunMode::Test)?;
        }
        Command::Package(args) => {
            run_build_like(&cli.root, &args, &build_config, cli.quiet, RunMode::Package)?;
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

fn to_build_options(args: &cli::BuildArgs) -> executor::BuildOptions {
    executor::BuildOptions {
        modules: args.modules.clone(),
        release: args.release,
        jobs: args.jobs,
        fail_fast: args.fail_fast,
    }
}

fn print_build_args(args: &cli::BuildArgs, effective: &executor::EffectiveSettings) {
    if args.modules.is_empty() {
        println!("  target: whole module graph");
    } else {
        println!("  target module(s): {}", args.modules.join(", "));
    }
    println!(
        "  configuration: {}",
        if effective.release {
            "Release"
        } else {
            "Debug"
        }
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

/// Which `executor` entry point [`run_build_like`] should call.
enum RunMode {
    Build,
    /// `coreforge test` doesn't have a real test adapter yet - see
    /// [`executor::test`].
    Test,
    Package,
}

/// Drives `Build`/`Test`/`Package`: resolves effective settings, either
/// prints the dry-run plan or runs the pipeline through `executor` with a
/// live progress display, then prints a summary and exits nonzero on
/// failure.
fn run_build_like(
    root: &camino::Utf8Path,
    args: &cli::BuildArgs,
    build_config: &Option<config::BuildSystemConfig>,
    quiet: bool,
    mode: RunMode,
) -> anyhow::Result<()> {
    let verb = match mode {
        RunMode::Build => "Build",
        RunMode::Test => "Test",
        RunMode::Package => "Package",
    };

    let options = to_build_options(args);
    let effective = executor::effective_settings(&options, build_config.as_ref());
    tracing::info!("{verb} command received.");
    print_build_args(args, &effective);

    if args.dry_run {
        tracing::info!("--dry-run: printing the build plan without running anything.");
        let plan = executor::dry_run(root, &options)?;
        if plan.order.is_empty() {
            tracing::info!("No modules found under {root}.");
            return Ok(());
        }
        println!("Build order (dependencies first):");
        for (i, id) in plan.order.iter().enumerate() {
            println!("  {}. {id}", i + 1);
        }
        println!("Parallel build levels:");
        for (level, ids) in plan.levels.iter().enumerate() {
            let names = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            println!("  level {level}: {names}");
        }
        return Ok(());
    }

    let cli_progress = CliProgress::new_if_active(quiet);
    let no_progress = scheduler::NoProgress;
    let progress: &dyn scheduler::ProgressSink = match &cli_progress {
        Some(p) => p,
        None => &no_progress,
    };

    let (outcome, dist_manifest) = match mode {
        RunMode::Build => (
            executor::build(root, &options, build_config.as_ref(), progress)?,
            None,
        ),
        RunMode::Test => {
            tracing::info!(
                "Test adapters are not implemented yet; using the scheduler dry-run runner."
            );
            (executor::test(root, &options, progress)?, None)
        }
        RunMode::Package => {
            let (outcome, manifest) =
                executor::package(root, &options, build_config.as_ref(), progress)?;
            (outcome, Some(manifest))
        }
    };

    if let Some(cli_progress) = &cli_progress {
        cli_progress.finish();
    } else {
        for job_outcome in &outcome.report.outcomes {
            print_outcome(job_outcome);
        }
    }

    let succeeded = outcome
        .report
        .outcomes
        .iter()
        .filter(|o| o.status.is_success())
        .count();
    let failed = outcome.report.failures().count();
    let skipped = outcome.report.skipped().count();
    println!("{verb} finished: {succeeded} succeeded, {failed} failed, {skipped} skipped.");

    if let Some(manifest) = &dist_manifest {
        print_dist_summary(root, &outcome.artifacts, manifest);
    }

    if !outcome.report.is_success() {
        std::process::exit(1);
    }

    Ok(())
}

fn print_dist_summary(
    root: &camino::Utf8Path,
    artifacts: &[toolchain::Artifact],
    manifest: &collector::DistManifest,
) {
    if artifacts.is_empty() {
        tracing::info!("No artifacts to collect.");
        return;
    }

    let dist_root = root.join("dist");
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

/// Runs the Project Inspector + Manifest parser + Build Graph over `root`
/// and prints the resolved modules, including any `coreforge.toml`
/// overrides and declared dependencies. Builds nothing.
fn run_resolve(root: &camino::Utf8Path) -> anyhow::Result<()> {
    tracing::info!("Resolving modules under: {root}");

    let modules = executor::inspect(root)?;
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

/// Resolves `root`'s full dependency graph and prints the build order and
/// parallel levels. Builds nothing.
fn run_graph(root: &camino::Utf8Path) -> anyhow::Result<()> {
    tracing::info!("Building dependency graph for: {root}");

    let graph = executor::resolve_graph(root)?;
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

fn run_workspace_sync(root: &camino::Utf8Path) -> anyhow::Result<()> {
    tracing::info!("Synchronizing workspace under: {root}");
    let lock = executor::workspace_sync(root)?;
    println!(
        "Workspace lock updated: {} Git repository/repositories pinned.",
        lock.resolved.len()
    );
    for repository in lock.resolved {
        println!("  - {}: {}", repository.name, repository.commit);
    }
    Ok(())
}

/// Cleans build outputs. Cleaning a single module only removes that
/// module's own managed build directory; cleaning everything (no module
/// given) also removes `dist/` - see [`executor::clean`].
fn run_clean(root: &camino::Utf8Path, module: Option<&str>) -> anyhow::Result<()> {
    let cleaned = executor::clean(root, module)?;
    for id in &cleaned {
        tracing::info!("Cleaned managed output for module '{id}'.");
    }
    if module.is_none() {
        tracing::info!("Cleaned dist/ directory.");
    }
    Ok(())
}
