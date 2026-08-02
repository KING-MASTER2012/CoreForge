//! Command Parser (Faz 0).
//!
//! Bu modul yalnizca komut satirini okur ve yapilandirilmis bir `Cli` degerine
//! cevirir. Henuz hicbir sey derlenmez, taranmaz ya da calistirilmaz -
//! Project Inspector (Faz 1) ve sonrasi bu isi devralacak.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "coreforge",
    version,
    about = "CoreForge - CoreVerse Engine icin build orchestrator",
    long_about = None
)]
pub struct Cli {
    /// Ayrintili (verbose) log ciktisi.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Kullanilacak build-system.toml dosyasinin yolu. Belirtilmezse repo
    /// kokunde otomatik aranir (Faz 8'de uygulanacak).
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Modulleri derler.
    Build(BuildArgs),

    /// Modullerin testlerini calistirir.
    Test(BuildArgs),

    /// Derlenen ciktilari paketler.
    Package(BuildArgs),

    /// Build ciktilarini (build/) temizler.
    Clean {
        /// Sadece belirtilen modulu temizle. Belirtilmezse tumu temizlenir.
        #[arg(value_name = "MODULE")]
        module: Option<String>,
    },
}

#[derive(Debug, clap::Args)]
pub struct BuildArgs {
    /// Sadece belirtilen modul(ler)i hedefle. Belirtilmezse tum modul grafi islenir.
    #[arg(value_name = "MODULE")]
    pub modules: Vec<String>,

    /// Release konfigurasyonu ile derle (varsayilan: Debug).
    #[arg(long)]
    pub release: bool,

    /// Hicbir komutu gercekten calistirma, sadece build plani goster.
    #[arg(long)]
    pub dry_run: bool,

    /// Paralel is sayisi. Belirtilmezse build-system.toml / CPU sayisi kullanilir.
    #[arg(short = 'j', long, value_name = "N")]
    pub jobs: Option<usize>,
}
