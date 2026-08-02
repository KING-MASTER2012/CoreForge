mod cli;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("[INFO] Verbose mode active");
    }
    if let Some(cfg) = &cli.config {
        eprintln!("[INFO] Config file: {}", cfg.display());
    }

    match cli.command {
        Command::Build(args) => {
            println!("[INFO] Build command received.");
            print_build_args(&args);
        }
        Command::Test(args) => {
            println!("[INFO] Test command received.");
            print_build_args(&args);
        }
        Command::Package(args) => {
            println!("[INFO] Package command received.");
            print_build_args(&args);
        }
        Command::Clean { module } => match module {
            Some(m) => println!("[INFO] Clean command received (module: {m})."),
            None => println!("[INFO] Clean command received (all modules)."),
        },
    }

    Ok(())
}

fn print_build_args(args: &cli::BuildArgs) {
    if args.modules.is_empty() {
        println!("  hedef: tum modul grafi");
    } else {
        println!("  hedef modul(ler): {}", args.modules.join(", "));
    }
    println!(
        "  konfigurasyon: {}",
        if args.release { "Release" } else { "Debug" }
    );
    println!("  dry-run: {}", args.dry_run);
    if let Some(jobs) = args.jobs {
        println!("  paralel is sayisi: {jobs}");
    }
}
