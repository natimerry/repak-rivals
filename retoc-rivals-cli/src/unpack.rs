use crate::archive;
use crate::cli::{UnpackArgs, UnpackDirArgs};
use crate::iostore_ops;
use crate::source::{classify_path, scan_directory_packages, IoStorePackage, PackageSource};
use crate::util::pak_aes_key;
use path_clean::PathClean;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

pub fn unpack(aes_key: retoc::AesKey, args: UnpackArgs) -> Result<(), String> {
    if args.output.is_some() && args.input.len() != 1 {
        return Err("--output can only be used with a single input".to_string());
    }

    let game_paks_dir = iostore_ops::resolve_game_paks_dir(&args.game_paks_dir)?;
    for input in &args.input {
        let output = args
            .output
            .clone()
            .unwrap_or_else(|| default_unpack_output(input));
        unpack_source(
            &aes_key,
            classify_path(input)?,
            &output,
            &args.filter,
            game_paks_dir.as_deref(),
            args.full_iostore_check,
            args.verbose,
            true,
        )?;
    }
    Ok(())
}

pub fn unpack_dir(aes_key: retoc::AesKey, args: UnpackDirArgs) -> Result<(), String> {
    let (iostore, legacy_paks, _) = scan_directory_packages(&args.input);
    if iostore.is_empty() && legacy_paks.is_empty() {
        return Err(format!("No packages found below {}", args.input.display()));
    }

    let game_paks_dir = iostore_ops::resolve_game_paks_dir(&args.game_paks_dir)?;
    let mut success_log = Vec::new();
    let mut failure_log = Vec::new();

    if !iostore.is_empty() {
        match iostore_ops::to_legacy_prefixed(
            &aes_key,
            &iostore,
            &args.output_prefix,
            &args.filter,
            game_paks_dir.as_deref(),
            args.full_iostore_check,
            args.verbose,
        ) {
            Ok(extracted) => {
                for item in extracted {
                    success_log.push((item.source, item.output));
                }
            }
            Err(error) => {
                for package in &iostore {
                    failure_log.push((package.utoc.clone(), error.clone()));
                }
            }
        }
    }

    for pak in legacy_paks {
        let output = pak.parent().unwrap_or_else(|| Path::new(".")).join(format!(
            "{}{}",
            args.output_prefix,
            pak.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("unpacked")
        ));
        match unpack_legacy_pak_to_dir(&pak, &output) {
            Ok(()) => success_log.push((pak, output)),
            Err(error) => failure_log.push((pak, error)),
        }
    }

    println!("\nExtraction summary");
    println!("Successful extractions: {}", success_log.len());
    for (src, dst) in &success_log {
        println!("{} -> {}", src.display(), dst.display());
    }
    println!("Failed extractions: {}", failure_log.len());
    for (path, reason) in &failure_log {
        println!("{} - {}", path.display(), reason);
    }

    if failure_log.is_empty() {
        Ok(())
    } else {
        Err("One or more extractions failed".to_string())
    }
}

pub fn unpack_legacy_pak_to_dir(pak_path: &Path, output: &Path) -> Result<(), String> {
    fs::create_dir_all(output)
        .map_err(|e| format!("Failed to create {}: {e}", output.display()))?;
    let output_root = output
        .canonicalize()
        .map_err(|e| format!("Failed to resolve {}: {e}", output.display()))?;

    let file =
        File::open(pak_path).map_err(|e| format!("Failed to open {}: {e}", pak_path.display()))?;
    let mut reader = BufReader::new(file);
    let pak = repak::PakBuilder::new()
        .key(pak_aes_key()?.0)
        .reader(&mut reader)
        .map_err(|e| format!("Failed to read legacy pak {}: {e}", pak_path.display()))?;

    let mount_point = PathBuf::from(pak.mount_point());
    let prefix = Path::new("../../../");
    let mut entries = pak.files();
    entries.sort();

    println!("Extracting {} to {}", pak_path.display(), output.display());
    for entry in entries {
        let full_path = mount_point.join(&entry);
        let rel_path = full_path.strip_prefix(prefix).map_err(|_| {
            format!(
                "Pak entry has unsupported mount path: {}",
                full_path.display()
            )
        })?;
        let out_path = output_root.join(rel_path).clean();
        if !out_path.starts_with(&output_root) {
            return Err(format!("Pak entry would write outside output: {}", entry));
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        let data = pak
            .get(&entry, &mut reader)
            .map_err(|e| format!("Failed to read pak entry {entry}: {e}"))?;
        File::create(&out_path)
            .and_then(|mut file| file.write_all(&data))
            .map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;
    }

    Ok(())
}

fn unpack_source(
    aes_key: &retoc::AesKey,
    source: PackageSource,
    output: &Path,
    filters: &[String],
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
    verbose: bool,
    flat_single: bool,
) -> Result<(), String> {
    match source {
        PackageSource::IoStore(package) => {
            iostore_ops::to_legacy_single(
                aes_key,
                &package,
                output,
                filters,
                game_paks_dir,
                full_iostore_check,
                verbose,
            )?;
            Ok(())
        }
        PackageSource::LegacyPak(path) => unpack_legacy_pak_to_dir(&path, output),
        PackageSource::RawDirectory(path) => Err(format!(
            "Input is a raw asset directory, not a package: {}",
            path.display()
        )),
        PackageSource::Archive(path) => {
            let temp = archive::extract_archive(&path)?;
            let root = archive_payload_root(temp.path());
            unpack_source(
                aes_key,
                classify_path(&root)?,
                output,
                filters,
                game_paks_dir,
                full_iostore_check,
                verbose,
                flat_single,
            )
        }
        PackageSource::DirectoryPackages {
            iostore,
            legacy_paks,
            archives: _,
            ..
        } => unpack_package_set(
            aes_key,
            &iostore,
            &legacy_paks,
            output,
            filters,
            game_paks_dir,
            full_iostore_check,
            verbose,
            flat_single,
        ),
    }
}

fn unpack_package_set(
    aes_key: &retoc::AesKey,
    iostore: &[IoStorePackage],
    legacy_paks: &[PathBuf],
    output_root: &Path,
    filters: &[String],
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
    verbose: bool,
    flat_single: bool,
) -> Result<(), String> {
    let total = iostore.len() + legacy_paks.len();
    if total == 0 {
        return Err("No packages found".to_string());
    }

    if iostore.len() == 1 && legacy_paks.is_empty() && flat_single {
        iostore_ops::to_legacy_single(
            aes_key,
            &iostore[0],
            output_root,
            filters,
            game_paks_dir,
            full_iostore_check,
            verbose,
        )?;
    } else if !iostore.is_empty() {
        iostore_ops::to_legacy_under_root(
            aes_key,
            iostore,
            output_root,
            filters,
            game_paks_dir,
            full_iostore_check,
            verbose,
        )?;
    }

    for pak in legacy_paks {
        let output = if total == 1 && flat_single {
            output_root.to_path_buf()
        } else {
            output_root.join(
                pak.file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("unpacked"),
            )
        };
        unpack_legacy_pak_to_dir(pak, &output)?;
    }

    Ok(())
}

fn default_unpack_output(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unpacked");
    input.parent().unwrap_or_else(|| Path::new(".")).join(stem)
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
