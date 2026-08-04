mod cli;

use clap::Parser;
use cli::{Cli, Command};
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
        Command::Build(args) => run_scheduled(&cli.root, &args, "Build")?,
        Command::Test(args) => run_scheduled(&cli.root, &args, "Test")?,
        Command::Package(args) => run_scheduled(&cli.root, &args, "Package")?,
        Command::Clean { module } => match module {
            Some(m) => println!("[INFO] Clean command received (module: {m})."),
            None => println!("[INFO] Clean command received (all modules)."),
        },
        Command::Inspect => {
            run_resolve(&cli.root)?;
        }
        Command::Graph => {
            run_graph(&cli.root)?;
        }
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

/// Runs the full pipeline (Inspector + Manifest + Build Graph + Scheduler,
/// Phases 1-4) over `root` and prints a per-module status report.
///
/// Real compilation (Phase 5's Tool Adapters) does not exist yet, so every
/// module is currently run through [`scheduler::DryRunRunner`],
/// which always succeeds immediately. This still exercises the Scheduler's
/// real dependency-aware, parallel-level logic end to end.
fn run_scheduled(root: &camino::Utf8Path, args: &cli::BuildArgs, verb: &str) -> anyhow::Result<()> {
    println!("[INFO] {verb} command received.");
    print_build_args(args);

    let inspector_config = InspectConfig::default();
    let graph = resolver::resolve(root, &inspector_config)?;

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

    println!(
        "[INFO] Tool Adapters are not implemented yet (Phase 5), so every module is currently \
         run through coreforge-scheduler's placeholder DryRunRunner (always succeeds)."
    );

    let config = scheduler::SchedulerConfig {
        parallel_jobs: args.jobs.unwrap_or(0),
        fail_fast: args.fail_fast,
    };
    let report = scheduler::run_build(&graph, &scheduler::DryRunRunner, &config)?;

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
    let modules = manifest::resolve_modules(root, &inspector_config)?;

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
    let graph = resolver::resolve(root, &inspector_config)?;

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
