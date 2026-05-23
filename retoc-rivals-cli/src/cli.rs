use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::RIVALS_AES_KEY;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// 256-bit AES key used for Marvel Rivals IoStore containers.
    #[arg(short, long, default_value = RIVALS_AES_KEY)]
    pub aes_key: retoc::AesKey,

    /// Increase log verbosity.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Print package info for legacy pak or IoStore.
    Info(InfoArgs),
    /// Build and print IoStore manifest data.
    Manifest(ManifestArgs),
    /// Extract packages, archives, or package directories.
    #[command(alias = "extract")]
    Unpack(UnpackArgs),
    /// Recursively extract every IoStore or legacy pak found below a directory.
    #[command(alias = "extract-dir")]
    UnpackDir(UnpackDirArgs),
    /// Package one explicit raw directory, legacy pak, archive, IoStore package, or package directory.
    Pack(PackArgs),
    /// Package every mod found below a mixed directory.
    PackDir(PackDirArgs),
    /// Patch raw assets in-place or rebuild installed IoStore mods with KawaiiPhysics porting.
    FixKawaiiPhysics(FixKawaiiPhysicsArgs),
}

#[derive(Parser, Debug)]
pub struct UnpackArgs {
    /// Input packages, archives, or package directories.
    #[arg(required = true)]
    pub input: Vec<PathBuf>,

    /// Output directory. Only valid with a single input. Defaults to a sibling directory named
    /// after the input file stem.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Verbose retoc output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Asset/package filters. IoStore only.
    #[arg(short, long)]
    pub filter: Vec<String>,

    /// Game Paks directory used for IoStore dependency containers.
    #[arg(long)]
    pub game_paks_dir: Option<PathBuf>,

    /// Open all game IoStore containers instead of only selected fast-path containers.
    #[arg(long)]
    pub full_iostore_check: bool,
}

#[derive(Parser, Debug)]
pub struct UnpackDirArgs {
    /// Directory to search for IoStore triples and legacy paks.
    pub input: PathBuf,

    /// Prefix for each generated output directory.
    #[arg(long, default_value = "unpacked_")]
    pub output_prefix: String,

    /// Verbose retoc output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Asset/package filters. Applied to every IoStore item.
    #[arg(short, long)]
    pub filter: Vec<String>,

    /// Game Paks directory used for IoStore dependency containers.
    #[arg(long)]
    pub game_paks_dir: Option<PathBuf>,

    /// Open all game IoStore containers instead of only selected fast-path containers.
    #[arg(long)]
    pub full_iostore_check: bool,
}

#[derive(Parser, Debug)]
pub struct PackArgs {
    /// Raw directories, legacy paks, archives, or IoStore package paths/directories.
    #[arg(required = true)]
    pub input: Vec<PathBuf>,

    /// Output directory. Defaults to each input directory's parent.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Mount point to write into the generated fake .pak.
    #[arg(long, default_value = "../../../")]
    pub mount_point: String,

    /// Path hash seed to write into the generated fake .pak.
    #[arg(long, default_value = "00000000")]
    pub path_hash_seed: String,

    /// Do not append the Marvel Rivals mod suffix to output names.
    #[arg(long)]
    pub no_mod_suffix: bool,

    /// Obfuscate generated IoStore containers.
    #[arg(long)]
    pub obfuscate: bool,

    /// IoStore compression method.
    #[arg(long, value_enum, default_value_t = CompressionArg::Oodle)]
    pub compression: CompressionArg,

    /// Port KawaiiPhysics assets while converting to IoStore.
    #[arg(long)]
    pub kawaii_physics: bool,

    /// USMAP used by KawaiiPhysics porting. If omitted, saved config is used, then the latest mapping is downloaded.
    #[arg(long)]
    pub kawaii_physics_usmap: Option<PathBuf>,

    /// Game Paks directory used when repacking IoStore with dependencies.
    #[arg(long)]
    pub game_paks_dir: Option<PathBuf>,

    /// Open all game IoStore containers instead of only selected fast-path containers.
    #[arg(long)]
    pub full_iostore_check: bool,
}

#[derive(Parser, Debug)]
pub struct PackDirArgs {
    /// Directory containing raw mod folders, legacy paks, archives, or IoStore package triples.
    pub input: PathBuf,

    /// Output directory. Defaults to the input directory's parent.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Mount point to write into generated fake .pak files.
    #[arg(long, default_value = "../../../")]
    pub mount_point: String,

    /// Path hash seed to write into generated fake .pak files.
    #[arg(long, default_value = "00000000")]
    pub path_hash_seed: String,

    /// Do not append the Marvel Rivals mod suffix to output names.
    #[arg(long)]
    pub no_mod_suffix: bool,

    /// Obfuscate generated IoStore containers.
    #[arg(long)]
    pub obfuscate: bool,

    /// IoStore compression method.
    #[arg(long, value_enum, default_value_t = CompressionArg::Oodle)]
    pub compression: CompressionArg,

    /// Port KawaiiPhysics assets while converting to IoStore.
    #[arg(long)]
    pub kawaii_physics: bool,

    /// USMAP used by KawaiiPhysics porting. If omitted, saved config is used, then the latest mapping is downloaded.
    #[arg(long)]
    pub kawaii_physics_usmap: Option<PathBuf>,

    /// Game Paks directory used when repacking IoStore with dependencies. If omitted, saved GUI config is used.
    #[arg(long)]
    pub game_paks_dir: Option<PathBuf>,

    /// Open all game IoStore containers instead of only selected fast-path containers.
    #[arg(long)]
    pub full_iostore_check: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompressionArg {
    None,
    Zlib,
    Zstd,
    Lz4,
    Oodle,
}

#[derive(Parser, Debug)]
pub struct InfoArgs {
    /// Package path, archive, or directory.
    pub input: PathBuf,
}

#[derive(Parser, Debug)]
pub struct ManifestArgs {
    /// IoStore .utoc/.ucas/.pak path or directory containing IoStore packages.
    pub input: PathBuf,

    /// Write manifest JSON to a file.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Print retoc to-legacy filter paths instead of manifest JSON.
    #[arg(long)]
    pub filters: bool,
}

#[derive(Parser, Debug)]
pub struct FixKawaiiPhysicsArgs {
    /// Optional unpacked/raw asset directory to patch in-place. If omitted, installed IoStore mods are rebuilt using saved GUI config.
    pub input: Option<PathBuf>,

    /// Directory for rebuilt mods.
    #[arg(short, long, default_value = "fixed-mods")]
    pub output: PathBuf,

    /// USMAP used by KawaiiPhysics porting. If omitted, saved config is used, then latest mapping is downloaded.
    #[arg(short, long)]
    pub usmap: Option<PathBuf>,
}
