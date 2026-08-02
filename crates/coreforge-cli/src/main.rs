mod cli;

use clap::Parser;
use cli::{Cli, Command};
use inspector::{InspectConfig, inspect_repository};

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
        Command::Build(args) => {
            println!("[INFO] Build command received.");
            print_build_args(&args);
            run_inspector(&cli.root)?;
        }
        Command::Test(args) => {
            println!("[INFO] Test command received.");
            print_build_args(&args);
            run_inspector(&cli.root)?;
        }
        Command::Package(args) => {
            println!("[INFO] Package command received.");
            print_build_args(&args);
            run_inspector(&cli.root)?;
        }
        Command::Clean { module } => match module {
            Some(m) => println!("[INFO] Clean command received (module: {m})."),
            None => println!("[INFO] Clean command received (all modules)."),
        },
        Command::Inspect => {
            run_inspector(&cli.root)?;
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
    if let Some(jobs) = args.jobs {
        println!("  parallel jobs: {jobs}");
    }
}

/// Runs the Project Inspector (Phase 1) over `root` and prints the discovered
/// modules. Note: this only *discovers* modules; nothing is built yet.
fn run_inspector(root: &camino::Utf8Path) -> anyhow::Result<()> {
    println!("[INFO] Inspecting repository: {root}");

    let config = InspectConfig::default();
    let modules = inspect_repository(root, &config)?;

    if modules.is_empty() {
        println!("[INFO] No modules found (no recognized marker files under {root}).");
        return Ok(());
    }

    println!("[INFO] Discovered {} module(s):", modules.len());
    for module in &modules {
        println!(
            "  - {:<24} [{:<6}]  {}",
            module.id, module.module_type, module.root
        );
    }

    Ok(())
}
