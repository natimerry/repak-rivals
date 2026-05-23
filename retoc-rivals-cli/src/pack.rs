use crate::archive;
use crate::cli::{PackArgs, PackDirArgs};
use crate::config::read_saved_state;
use crate::iostore_ops;
use crate::kawaii_utils;
use crate::source::{classify_path, scan_directory_packages, IoStorePackage, PackageSource};
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
use tempfile::TempDir;

struct ExtractedArchive {
    _temp: TempDir,
    source: PackageSource,
    source_name: String,
}

pub fn pack(aes_key: retoc::AesKey, mut args: PackArgs) -> Result<(), String> {
    if args.kawaii_physics {
        args.kawaii_physics_usmap = Some(resolve_pack_usmap(args.kawaii_physics_usmap.as_deref())?);
    }

    let game_paks_dir = iostore_ops::resolve_game_paks_dir(&args.game_paks_dir)?;
    for input in &args.input {
        tracing::info!(input = %input.display(), "Classifying pack input");
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

pub fn pack_dir(aes_key: retoc::AesKey, args: PackDirArgs) -> Result<(), String> {
    if !args.input.is_dir() {
        return Err(format!(
            "Input is not a directory: {}",
            args.input.display()
        ));
    }

    let mut pack_args = PackArgs {
        input: Vec::new(),
        output: args.output.clone(),
        mount_point: args.mount_point,
        path_hash_seed: args.path_hash_seed,
        no_mod_suffix: args.no_mod_suffix,
        obfuscate: args.obfuscate,
        compression: args.compression,
        kawaii_physics: args.kawaii_physics,
        kawaii_physics_usmap: args.kawaii_physics_usmap,
        game_paks_dir: args.game_paks_dir,
        full_iostore_check: args.full_iostore_check,
    };
    if pack_args.kawaii_physics {
        pack_args.kawaii_physics_usmap = Some(resolve_pack_usmap(
            pack_args.kawaii_physics_usmap.as_deref(),
        )?);
    }

    let output_dir = pack_args.output.clone().unwrap_or_else(|| {
        args.input
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    });
    let default_output = output_dir.clone();
    let game_paks_dir = iostore_ops::resolve_game_paks_dir(&pack_args.game_paks_dir)?;
    tracing::info!(input = %args.input.display(), "Scanning mixed pack directory");
    println!(
        "Scanning {} for mods, packages, and archives",
        args.input.display()
    );
    let (iostore, legacy_paks, archives) = scan_directory_packages(&args.input);
    let raw_dirs = scan_raw_mod_dirs(&args.input);
    let raw_roots = raw_dirs
        .iter()
        .map(|path| path.as_path())
        .collect::<Vec<_>>();

    let iostore = iostore
        .into_iter()
        .filter(|package| !is_under_any(&package.utoc, &raw_roots))
        .collect::<Vec<_>>();
    let legacy_paks = legacy_paks
        .into_iter()
        .filter(|pak| !is_under_any(pak, &raw_roots))
        .collect::<Vec<_>>();
    let archives = archives
        .into_iter()
        .filter(|archive| !is_under_any(archive, &raw_roots))
        .collect::<Vec<_>>();

    tracing::info!(
        raw_dirs = raw_dirs.len(),
        iostore = iostore.len(),
        legacy_paks = legacy_paks.len(),
        archives = archives.len(),
        "Finished scanning mixed pack directory"
    );
    let item_count = raw_dirs.len() + iostore.len() + legacy_paks.len() + archives.len();
    if item_count == 0 {
        return Err(format!(
            "No packable mods found below {}",
            args.input.display()
        ));
    }

    println!(
        "Found {item_count} packable mods below {}",
        args.input.display()
    );
    for raw_dir in &raw_dirs {
        pack_raw_dir(
            &aes_key,
            &pack_args,
            raw_dir,
            &input_stem(raw_dir),
            &default_output,
        )?;
    }
    pack_discovered_items(
        &aes_key,
        &pack_args,
        iostore,
        legacy_paks,
        archives,
        &default_output,
        game_paks_dir.as_deref(),
    )?;

    Ok(())
}

fn resolve_pack_usmap(current: Option<&Path>) -> Result<PathBuf, String> {
    let saved_usmap = if current.is_none() {
        read_saved_state()
            .ok()
            .and_then(|state| state.kawaii_physics_usmap)
    } else {
        None
    };
    kawaii_utils::resolve_kawaii_usmap(current.or(saved_usmap.as_deref()))
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
            archives,
        } => {
            tracing::info!(
                root = %root.display(),
                iostore = iostore.len(),
                legacy_paks = legacy_paks.len(),
                archives = archives.len(),
                "Packing discovered directory packages"
            );
            let output_dir = args.output.clone().unwrap_or_else(|| {
                root.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            });
            pack_discovered_items(
                aes_key,
                args,
                iostore,
                legacy_paks,
                archives,
                &output_dir,
                game_paks_dir,
            )
        }
        PackageSource::Archive(path) => {
            tracing::info!(archive = %path.display(), "Packing archive input");
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

fn pack_discovered_items(
    aes_key: &retoc::AesKey,
    args: &PackArgs,
    mut iostore: Vec<IoStorePackage>,
    mut legacy_paks: Vec<PathBuf>,
    archives: Vec<PathBuf>,
    default_output: &Path,
    game_paks_dir: Option<&Path>,
) -> Result<(), String> {
    let mut archive_sources = Vec::new();
    let mut archive_raw_dirs = Vec::new();
    if !archives.is_empty() {
        tracing::info!(
            archive_count = archives.len(),
            "Extracting discovered archives"
        );
        println!("Extracting {} archive(s)", archives.len());
    }
    for archive in archives {
        println!("Extracting archive {}", archive.display());
        let extracted = extract_archive_source(&archive)?;
        collect_archive_source(
            &extracted.source,
            &extracted.source_name,
            &mut iostore,
            &mut legacy_paks,
            &mut archive_raw_dirs,
        );
        tracing::debug!(
            archive = %archive.display(),
            iostore = iostore.len(),
            legacy_paks = legacy_paks.len(),
            raw_dirs = archive_raw_dirs.len(),
            "Collected archive payload"
        );
        archive_sources.push(extracted);
    }

    pack_iostore_packages(aes_key, args, &iostore, default_output, game_paks_dir)?;
    for pak in &legacy_paks {
        repack_legacy_pak(aes_key, args, pak, default_output)?;
    }
    for (path, name) in &archive_raw_dirs {
        pack_raw_dir(aes_key, args, path, name, default_output)?;
    }

    drop(archive_sources);
    Ok(())
}

fn extract_archive_source(path: &Path) -> Result<ExtractedArchive, String> {
    let temp = archive::extract_archive(path)?;
    let root = archive_payload_root(temp.path());
    let source = classify_path(&root)?;
    Ok(ExtractedArchive {
        _temp: temp,
        source,
        source_name: input_stem(path),
    })
}

fn collect_archive_source(
    source: &PackageSource,
    source_name: &str,
    iostore: &mut Vec<IoStorePackage>,
    legacy_paks: &mut Vec<PathBuf>,
    raw_dirs: &mut Vec<(PathBuf, String)>,
) {
    match source {
        PackageSource::IoStore(package) => iostore.push(package.clone()),
        PackageSource::LegacyPak(path) => legacy_paks.push(path.clone()),
        PackageSource::RawDirectory(path) => raw_dirs.push((path.clone(), source_name.to_string())),
        PackageSource::DirectoryPackages {
            iostore: packages,
            legacy_paks: paks,
            archives: _,
            ..
        } => {
            iostore.extend(packages.iter().cloned());
            legacy_paks.extend(paks.iter().cloned());
        }
        PackageSource::Archive(_) => {}
    }
}

fn pack_iostore_packages(
    aes_key: &retoc::AesKey,
    args: &PackArgs,
    packages: &[IoStorePackage],
    default_output: &Path,
    game_paks_dir: Option<&Path>,
) -> Result<(), String> {
    if packages.is_empty() {
        return Ok(());
    }

    let output_dir = args
        .output
        .clone()
        .unwrap_or_else(|| default_output.to_path_buf());

    if !args.kawaii_physics {
        for package in packages {
            let output =
                iostore_ops::copy_iostore_package(package, &output_dir, args.no_mod_suffix)?;
            println!("Installed IoStore package to {}", output.display());
        }
        return Ok(());
    }

    let game_paks_dir = game_paks_dir.ok_or_else(|| {
        "Game Paks directory is required when repacking IoStore mods with --kawaii-physics. Pass --game-paks-dir or open repak-gui once so its saved config can be used.".to_string()
    })?;
    let temp = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let outputs = packages
        .iter()
        .map(|package| temp.path().join(package.stem()))
        .collect::<Vec<_>>();
    let extracted = iostore_ops::to_legacy_outputs(
        aes_key,
        packages,
        outputs,
        &[],
        Some(game_paks_dir),
        args.full_iostore_check,
        true,
    )?;

    for (package, extracted) in packages.iter().zip(extracted) {
        pack_raw_dir(
            aes_key,
            args,
            &extracted.output,
            &package.stem(),
            &output_dir,
        )?;
    }
    Ok(())
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

    let game_paks_dir = game_paks_dir.ok_or_else(|| {
        "Game Paks directory is required when repacking IoStore mods with --kawaii-physics. Pass --game-paks-dir or open repak-gui once so its saved config can be used.".to_string()
    })?;

    let temp = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let extracted_dir = temp.path().join(package.stem());
    iostore_ops::to_legacy_single(
        aes_key,
        package,
        &extracted_dir,
        &[],
        Some(game_paks_dir),
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

fn scan_raw_mod_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = fs::read_dir(root)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir() && has_uasset(path))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

fn has_uasset(dir: &Path) -> bool {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("uasset"))
        })
}

fn is_under_any(path: &Path, roots: &[&Path]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
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
