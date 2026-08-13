mod archive;
mod cli;
mod config;
mod info;
mod iostore_ops;
mod kawaii_utils;
mod legacy;
mod manifest;
mod pack;
mod source;
mod unpack;
mod util;

use clap::Parser;
use cli::{Args, Command};

pub const RIVALS_AES_KEY: &str = "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74";
pub const MOD_NAME_SUFFIX: &str = "_9999999_P";

fn main() {
    let args = Args::parse_from(cli::normalize_windows_output_args(std::env::args_os()));
    let verbosity = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    init_tracing(verbosity);

    if let Err(error) = run(args) {
        tracing::error!("{error}");
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let aes_key_override = args.aes_key;
    let aes_key = match aes_key_override.clone() {
        Some(key) => key,
        None => RIVALS_AES_KEY
            .parse()
            .map_err(|error| format!("Invalid built-in AES key: {error}"))?,
    };

    match args.command {
        Command::Info(command) => info::info(aes_key, command),
        Command::Manifest(command) => manifest::manifest(aes_key, command),
        Command::Unpack(command) => unpack::unpack(aes_key, command),
        Command::UnpackDir(command) => unpack::unpack_dir(aes_key, command),
        Command::Pack(command) => pack::pack(
            pack::PackCrypto::new(aes_key, aes_key_override, args.guid),
            command,
        ),
        Command::PackDir(command) => pack::pack_dir(
            pack::PackCrypto::new(aes_key, aes_key_override, args.guid),
            command,
        ),
        Command::FixKawaiiPhysics(command) => legacy::fix_kawaii_physics(
            pack::PackCrypto::new(aes_key, aes_key_override, args.guid),
            command,
        ),
    }
}

fn init_tracing(verbosity: tracing::Level) {
    tracing_subscriber::fmt()
        .with_max_level(verbosity)
        .with_target(false)
        .without_time()
        .init();
}
