use crate::archive;
use crate::cli::InfoArgs;
use crate::config::retoc_config;
use crate::source::{classify_path, IoStorePackage, PackageSource};
use crate::util::pak_aes_key;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub fn info(aes_key: retoc::AesKey, args: InfoArgs) -> Result<(), String> {
    print_source_info(&aes_key, classify_path(&args.input)?)
}

fn print_source_info(aes_key: &retoc::AesKey, source: PackageSource) -> Result<(), String> {
    match source {
        PackageSource::IoStore(package) => print_iostore_info(aes_key, &package),
        PackageSource::LegacyPak(path) => print_legacy_pak_info(&path),
        PackageSource::RawDirectory(path) => {
            println!("type: raw-directory");
            println!("path: {}", path.display());
            Ok(())
        }
        PackageSource::Archive(path) => {
            println!("type: archive");
            println!("path: {}", path.display());
            let temp = archive::extract_archive(&path)?;
            let root = archive_payload_root(temp.path());
            println!("payload: {}", root.display());
            print_source_info(aes_key, classify_path(&root)?)
        }
        PackageSource::DirectoryPackages {
            root,
            iostore,
            legacy_paks,
        } => {
            println!("type: package-directory");
            println!("path: {}", root.display());
            println!("iostore packages: {}", iostore.len());
            for package in &iostore {
                println!("  {}", package.utoc.display());
            }
            println!("legacy pak files: {}", legacy_paks.len());
            for pak in &legacy_paks {
                println!("  {}", pak.display());
            }
            Ok(())
        }
    }
}

fn print_iostore_info(aes_key: &retoc::AesKey, package: &IoStorePackage) -> Result<(), String> {
    println!("type: iostore");
    println!("pak: {}", package.pak.display());
    println!("utoc: {}", package.utoc.display());
    println!("ucas: {}", package.ucas.display());
    let store = retoc::open_iostore(&package.utoc, retoc_config(aes_key.clone()))
        .map_err(|e| format!("Failed to open {}: {e}", package.utoc.display()))?;
    store.print_info(0);
    Ok(())
}

fn print_legacy_pak_info(path: &Path) -> Result<(), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    let mut reader = BufReader::new(file);
    let pak = repak::PakBuilder::new()
        .key(pak_aes_key()?.0)
        .reader(&mut reader)
        .map_err(|e| format!("Failed to read legacy pak {}: {e}", path.display()))?;

    println!("type: legacy-pak");
    println!("path: {}", path.display());
    println!("version: {:?}", pak.version());
    println!("mount point: {}", pak.mount_point());
    println!("encrypted index: {}", pak.encrypted_index());
    println!(
        "encryption guid: {}",
        pak.encryption_guid()
            .map(|guid| format!("{guid:032x}"))
            .unwrap_or_else(|| "<none>".to_string())
    );
    println!(
        "path hash seed: {}",
        pak.path_hash_seed()
            .map(|seed| format!("{seed:016x}"))
            .unwrap_or_else(|| "<none>".to_string())
    );
    println!("files: {}", pak.files().len());
    Ok(())
}

fn archive_payload_root(root: &Path) -> PathBuf {
    let Ok(entries) = std::fs::read_dir(root) else {
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
