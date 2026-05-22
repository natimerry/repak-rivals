use crate::archive;
use crate::cli::PackArgs;
use crate::iostore_ops;
use crate::kawaii_utils;
use crate::source::{classify_path, IoStorePackage, PackageSource};
use crate::unpack::unpack_legacy_pak_to_dir;
use crate::util::{
    collect_files, ensure_mod_name_suffix, pak_aes_key, parse_path_hash_seed, repak_compression,
    retoc_compression,
};
use retoc::{action_to_zen, ActionToZen, Config, EngineVersion, FGuid};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn pack(aes_key: retoc::AesKey, mut args: PackArgs) -> Result<(), String> {
    if args.kawaii_physics || args.kawaii_physics_only {
        args.kawaii_physics_usmap = Some(kawaii_utils::resolve_kawaii_usmap(
            args.kawaii_physics_usmap.as_deref(),
        )?);
    }

    if args.kawaii_physics_only {
        let usmap = args
            .kawaii_physics_usmap
            .as_deref()
            .expect("USMAP must be resolved when kawaii_physics_only is set");
        for input in &args.input {
            fix_kawaii_physics_directory(input, usmap)?;
        }
        return Ok(());
    }

    let game_paks_dir = iostore_ops::resolve_game_paks_dir(&args.game_paks_dir)?;
    for input in &args.input {
        let source = classify_path(input)?;
        let default_output = input
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        pack_source(
            &aes_key,
            &args,
            source,
            input_stem(input),
            &default_output,
            game_paks_dir.as_deref(),
        )?;
    }
    Ok(())
}

fn fix_kawaii_physics_directory(input: &Path, usmap: &Path) -> Result<(), String> {
    if !input.is_dir() {
        return Err(format!("Input is not a directory: {}", input.display()));
    }

    tracing::info!(input = %input.display(), usmap = %usmap.display(), "Porting KawaiiPhysics assets in-place");
    println!("Fixing KawaiiPhysics assets in {}", input.display());
    let ported = retoc::port_kawaii_physics_directory(input, usmap, true)
        .map_err(|e| format!("KawaiiPhysics directory fix failed: {e}"))?;
    println!("Ported {ported} KawaiiPhysics anim nodes");
    Ok(())
}

fn pack_source(
    aes_key: &retoc::AesKey,
    args: &PackArgs,
    source: PackageSource,
    source_name: String,
    default_output: &Path,
    game_paks_dir: Option<&Path>,
) -> Result<(), String> {
    match source {
        PackageSource::RawDirectory(path) => {
            pack_raw_dir(aes_key, args, &path, &source_name, default_output)
        }
        PackageSource::LegacyPak(path) => repack_legacy_pak(aes_key, args, &path, default_output),
        PackageSource::IoStore(package) => {
            pack_iostore_package(aes_key, args, &package, default_output, game_paks_dir)
        }
        PackageSource::DirectoryPackages {
            root,
            iostore,
            legacy_paks,
        } => {
            let output_dir = args.output.clone().unwrap_or_else(|| {
                root.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            });
            for package in &iostore {
                pack_iostore_package(aes_key, args, package, &output_dir, game_paks_dir)?;
            }
            for pak in &legacy_paks {
                repack_legacy_pak(aes_key, args, pak, &output_dir)?;
            }
            Ok(())
        }
        PackageSource::Archive(path) => {
            let temp = archive::extract_archive(&path)?;
            let root = archive_payload_root(temp.path());
            pack_source(
                aes_key,
                args,
                classify_path(&root)?,
                input_stem(&path),
                default_output,
                game_paks_dir,
            )
        }
    }
}

fn pack_iostore_package(
    aes_key: &retoc::AesKey,
    args: &PackArgs,
    package: &IoStorePackage,
    default_output: &Path,
    game_paks_dir: Option<&Path>,
) -> Result<(), String> {
    let output_dir = args
        .output
        .clone()
        .unwrap_or_else(|| default_output.to_path_buf());

    if !args.kawaii_physics {
        let output = iostore_ops::copy_iostore_package(package, &output_dir, args.no_mod_suffix)?;
        println!("Installed IoStore package to {}", output.display());
        return Ok(());
    }

    let temp = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let extracted_dir = temp.path().join(package.stem());
    iostore_ops::to_legacy_single(
        aes_key,
        package,
        &extracted_dir,
        &[],
        game_paks_dir,
        args.full_iostore_check,
        true,
    )?;
    pack_raw_dir(aes_key, args, &extracted_dir, &package.stem(), &output_dir)
}

fn repack_legacy_pak(
    aes_key: &retoc::AesKey,
    args: &PackArgs,
    pak_path: &Path,
    default_output: &Path,
) -> Result<(), String> {
    let temp = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    unpack_legacy_pak_to_dir(pak_path, temp.path())?;
    pack_raw_dir(
        aes_key,
        args,
        temp.path(),
        &input_stem(pak_path),
        default_output,
    )
}

fn pack_raw_dir(
    aes_key: &retoc::AesKey,
    args: &PackArgs,
    input: &Path,
    raw_name: &str,
    default_output: &Path,
) -> Result<(), String> {
    if !input.is_dir() {
        return Err(format!("Input is not a directory: {}", input.display()));
    }

    let output_dir = args
        .output
        .clone()
        .unwrap_or_else(|| default_output.to_path_buf());
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create {}: {e}", output_dir.display()))?;

    let mod_name = if args.no_mod_suffix {
        raw_name.to_string()
    } else {
        ensure_mod_name_suffix(raw_name)
    };
    let utoc = output_dir.join(format!("{mod_name}.utoc"));

    let mut action = ActionToZen::new(
        input.to_path_buf(),
        utoc,
        EngineVersion::UE5_3,
        retoc_compression(args.compression),
    )
    .with_obfuscation(args.obfuscate);
    if let Some(usmap) = args.kawaii_physics_usmap.clone() {
        action = action.with_kawaii_physics_port(usmap);
    }

    tracing::info!(input = %input.display(), output = %output_dir.display(), "Packing directory into IoStore");
    println!("Packing {} to {}", input.display(), output_dir.display());
    let mut config = Config {
        container_header_version_override: None,
        ..Default::default()
    };
    config.aes_keys.insert(FGuid::default(), aes_key.clone());
    config.port_kawaii_physics = args.kawaii_physics;
    config.kawaii_physics_usmap = args.kawaii_physics_usmap.clone();
    config.kawaii_physics_force_rebuild = true;
    action_to_zen(action, Arc::new(config)).map_err(|e| format!("Pack failed: {e}"))?;

    write_chunknames_pak(
        input,
        &output_dir.join(format!("{mod_name}.pak")),
        &args.mount_point,
        &args.path_hash_seed,
        args.compression,
    )
}

fn write_chunknames_pak(
    input: &Path,
    output: &Path,
    mount_point: &str,
    path_hash_seed: &str,
    compression: crate::cli::CompressionArg,
) -> Result<(), String> {
    let mut paths = Vec::new();
    collect_files(&mut paths, input).map_err(|e| format!("Failed to scan input files: {e}"))?;
    let mut rel_paths = paths
        .iter()
        .map(|path| {
            path.strip_prefix(input)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .map_err(|e| format!("File is not in input directory: {} ({e})", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    rel_paths.sort();

    let seed = parse_path_hash_seed(path_hash_seed)?;
    let builder = repak::PakBuilder::new()
        .compression(repak_compression(compression))
        .key(pak_aes_key()?.0);
    let file =
        File::create(output).map_err(|e| format!("Failed to create {}: {e}", output.display()))?;
    let mut pak = builder.writer(
        BufWriter::new(file),
        repak::Version::V11,
        mount_point.to_string(),
        Some(seed),
    );
    let entry = pak
        .entry_builder()
        .build_entry(true, rel_paths.join("\n").into_bytes(), "chunknames")
        .map_err(|e| format!("Failed to build chunknames entry: {e}"))?;
    pak.write_entry("chunknames".to_string(), entry)
        .map_err(|e| format!("Failed to write chunknames entry: {e}"))?;
    pak.write_index()
        .map_err(|e| format!("Failed to write pak index: {e}"))?;
    println!("Wrote {}", output.display());
    Ok(())
}

fn input_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("mod")
        .to_string()
}

fn archive_payload_root(root: &Path) -> PathBuf {
    let Ok(entries) = fs::read_dir(root) else {
        return root.to_path_buf();
    };
    let mut dirs = Vec::new();
    let mut files = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else {
            files += 1;
        }
    }
    if files == 0 && dirs.len() == 1 {
        dirs.pop().unwrap()
    } else {
        root.to_path_buf()
    }
}
