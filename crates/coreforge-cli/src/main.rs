mod cli;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        eprintln!("[INFO] verbose mod aktif");
    }
    if let Some(cfg) = &cli.config {
        eprintln!("[INFO] config dosyasi: {}", cfg.display());
    }

    // Faz 0: sadece komut okunur, henuz hicbir sey derlenmez.
    // Project Inspector (Faz 1) ve sonrasi burada devreye girecek.
    match cli.command {
        Command::Build(args) => {
            println!("[INFO] Build komutu alindi.");
            print_build_args(&args);
        }
        Command::Test(args) => {
            println!("[INFO] Test komutu alindi.");
            print_build_args(&args);
        }
        Command::Package(args) => {
            println!("[INFO] Package komutu alindi.");
            print_build_args(&args);
        }
        Command::Clean { module } => {
            match module {
                Some(m) => println!("[INFO] Clean komutu alindi (modul: {m})."),
                None => println!("[INFO] Clean komutu alindi (tum moduller)."),
            }
        }
    }

    Ok(())
}

fn print_build_args(args: &cli::BuildArgs) {
    if args.modules.is_empty() {
        println!("  hedef: tum modul grafi");
    } else {
        println!("  hedef modul(ler): {}", args.modules.join(", "));
    }
    println!("  konfigurasyon: {}", if args.release { "Release" } else { "Debug" });
    println!("  dry-run: {}", args.dry_run);
    if let Some(jobs) = args.jobs {
        println!("  paralel is sayisi: {jobs}");
    }
}
