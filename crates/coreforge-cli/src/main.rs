mod cli;

use std::collections::HashMap;

use camino::Utf8PathBuf;
use clap::Parser;
use cli::{Cli, Command};
use coreforge_core::ModuleId;
use inspector::InspectConfig;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("[INFO] verbose mode enabled");
        eprintln!("[INFO] repository root: {}", cli.root);
    }
    if let Some(cfg) = &cli.config {
        eprintln!("[INFO] config file: {cfg}");
    }

    match cli.command {
        Command::Build(args) => run_scheduled(&cli.root, &args, "Build", true)?,
        Command::Test(args) => run_scheduled(&cli.root, &args, "Test", false)?,
        Command::Package(args) => run_scheduled(&cli.root, &args, "Package", false)?,
        Command::Clean { module } => run_clean(&cli.root, module.as_deref())?,
        Command::Inspect => {
            run_resolve(&cli.root)?;
        }
        Command::Graph => {
            run_graph(&cli.root)?;
        }
        Command::Workspace(args) => match args.command {
            cli::WorkspaceCommand::Sync => run_workspace_sync(&cli.root)?,
        },
    }

    Ok(())
}

fn print_build_args(args: &cli::BuildArgs) {
    if args.modules.is_empty() {
        println!("  target: whole module graph");
    } else {
        println!("  target module(s): {}", args.modules.join(", "));
    }
    println!(
        "  configuration: {}",
        if args.release { "Release" } else { "Debug" }
    );
    println!("  dry-run: {}", args.dry_run);
    println!("  fail-fast: {}", args.fail_fast);
    if let Some(jobs) = args.jobs {
        println!("  parallel jobs: {jobs}");
    }
}

/// Runs the full pipeline over `root` and prints a per-module status report.
/// `Build` uses the Phase 5 toolchain adapters; test and package adapters are
/// intentionally still scheduler dry-runs until their command semantics land.
fn run_scheduled(
    root: &camino::Utf8Path,
    args: &cli::BuildArgs,
    verb: &str,
    use_toolchain: bool,
) -> anyhow::Result<()> {
    println!("[INFO] {verb} command received.");
    print_build_args(args);

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
        println!("[INFO] No modules found under {root}.");
        return Ok(());
    }

    if args.dry_run {
        println!("[INFO] --dry-run: printing the build plan without running anything.");
        println!("[INFO] Build order (dependencies first):");
        for (i, id) in graph.build_order()?.iter().enumerate() {
            println!("  {}. {id}", i + 1);
        }
        println!("[INFO] Parallel build levels:");
        for (level, ids) in graph.build_levels()?.iter().enumerate() {
            let names = ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            println!("  level {level}: {names}");
        }
        return Ok(());
    }

    let config = scheduler::SchedulerConfig {
        parallel_jobs: args.jobs.unwrap_or(0),
        fail_fast: args.fail_fast,
    };
    let report = if use_toolchain {
        let contexts = build_contexts(&project.module_dirs, root, args.release);
        let runner = toolchain::ToolchainRunner::new(contexts);
        let report = scheduler::run_build(graph, &runner, &config)?;
        println!(
            "[INFO] Build produced {} managed artifact directory/directories.",
            runner.artifacts().len()
        );
        report
    } else {
        println!(
            "[INFO] {verb} tool adapters are not implemented yet; using the scheduler dry-run runner."
        );
        scheduler::run_build(graph, &scheduler::DryRunRunner, &config)?
    };

    for outcome in &report.outcomes {
        let (tag, detail) = match &outcome.status {
            scheduler::JobStatus::Success => ("OK", String::new()),
            scheduler::JobStatus::Failed(reason) => ("FAIL", reason.clone()),
            scheduler::JobStatus::Skipped(reason) => ("SKIP", reason.clone()),
        };
        println!(
            "  [{tag:<4}] {:<24} {:>7.2?}  {detail}",
            outcome.module, outcome.duration
        );
    }

    let succeeded = report
        .outcomes
        .iter()
        .filter(|o| o.status.is_success())
        .count();
    let failed = report.failures().count();
    let skipped = report.skipped().count();
    println!("[INFO] {verb} finished: {succeeded} succeeded, {failed} failed, {skipped} skipped.");

    if !report.is_success() {
        std::process::exit(1);
    }

    Ok(())
}

/// Runs the Project Inspector (Phase 1) + Manifest parser (Phase 2) over
/// `root` and prints the resolved modules, including any `coreforge.toml`
/// overrides and declared dependencies. Note: this only *resolves* modules;
/// nothing is built yet.
fn run_resolve(root: &camino::Utf8Path) -> anyhow::Result<()> {
    println!("[INFO] Resolving modules under: {root}");

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
        println!("[INFO] No modules found under {root}.");
        return Ok(());
    }

    println!("[INFO] Resolved {} module(s):", modules.len());
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
    println!("[INFO] Building dependency graph for: {root}");

    let inspector_config = InspectConfig::default();
    let graph = resolve_graph(root, &inspector_config)?;

    if graph.is_empty() {
        println!("[INFO] No modules found under {root}.");
        return Ok(());
    }

    println!("[INFO] {} module(s) in graph.", graph.len());

    println!("[INFO] Build order (dependencies first):");
    for (i, id) in graph.build_order()?.iter().enumerate() {
        println!("  {}. {id}", i + 1);
    }

    println!("[INFO] Parallel build levels:");
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
                    output_dir: build_root.join(module_output_name(id)),
                    profile,
                },
            )
        })
        .collect()
}

fn module_output_name(id: &ModuleId) -> String {
    id.0.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn run_workspace_sync(root: &camino::Utf8Path) -> anyhow::Result<()> {
    println!("[INFO] Synchronizing workspace under: {root}");
    let lock = coreforge_workspace::sync(root)?;
    println!(
        "[INFO] Workspace lock updated: {} Git repository/repositories pinned.",
        lock.resolved.len()
    );
    for repository in lock.resolved {
        println!("  - {}: {}", repository.name, repository.commit);
    }
    Ok(())
}

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
        println!("[INFO] Cleaned managed output for module '{}'.", module.id);
    }
    Ok(())
}
