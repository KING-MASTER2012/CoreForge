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
        Command::Build(args) => {
            println!("[INFO] Build command received.");
            print_build_args(&args);
            run_resolve(&cli.root)?;
        }
        Command::Test(args) => {
            println!("[INFO] Test command received.");
            print_build_args(&args);
            run_resolve(&cli.root)?;
        }
        Command::Package(args) => {
            println!("[INFO] Package command received.");
            print_build_args(&args);
            run_resolve(&cli.root)?;
        }
        Command::Clean { module } => match module {
            Some(m) => println!("[INFO] Clean command received (module: {m})."),
            None => println!("[INFO] Clean command received (all modules)."),
        },
        Command::Inspect => {
            run_resolve(&cli.root)?;
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
