use crate::archive;
use crate::cli::ManifestArgs;
use crate::iostore_ops;
use crate::source::{classify_path, IoStorePackage, PackageSource};
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn manifest(aes_key: retoc::AesKey, args: ManifestArgs) -> Result<(), String> {
    let content = match classify_path(&args.input)? {
        PackageSource::IoStore(package) => emit_manifest(&aes_key, &[package], args.filters)?,
        PackageSource::DirectoryPackages { iostore, .. } => {
            emit_manifest(&aes_key, &iostore, args.filters)?
        }
        PackageSource::Archive(path) => {
            let temp = archive::extract_archive(&path)?;
            let root = archive_payload_root(temp.path());
            match classify_path(&root)? {
                PackageSource::IoStore(package) => {
                    emit_manifest(&aes_key, &[package], args.filters)?
                }
                PackageSource::DirectoryPackages { iostore, .. } => {
                    emit_manifest(&aes_key, &iostore, args.filters)?
                }
                other => {
                    return Err(format!(
                        "Archive did not contain IoStore packages: {}",
                        source_kind(&other)
                    ));
                }
            }
        }
        other => {
            return Err(format!(
                "Manifest requires IoStore package input, got {}",
                source_kind(&other)
            ));
        }
    };

    if let Some(output) = args.output {
        fs::write(&output, content)
            .map_err(|e| format!("Failed to write {}: {e}", output.display()))?;
    } else {
        println!("{content}");
    }

    Ok(())
}

fn emit_manifest(
    aes_key: &retoc::AesKey,
    packages: &[IoStorePackage],
    filters: bool,
) -> Result<String, String> {
    if packages.is_empty() {
        return Err("No IoStore packages found".to_string());
    }

    if filters {
        let mut lines = Vec::new();
        for package in packages {
            if packages.len() > 1 {
                lines.push(format!("# {}", package.utoc.display()));
            }
            lines.extend(iostore_ops::manifest_filter(aes_key, &package.utoc)?);
        }
        return Ok(lines.join("\n"));
    }

    if packages.len() == 1 {
        let value = iostore_ops::manifest_value(aes_key, &packages[0].utoc)?;
        return serde_json::to_string_pretty(&value)
            .map_err(|e| format!("Failed to serialize manifest JSON: {e}"));
    }

    let mut manifests = Vec::new();
    for package in packages {
        manifests.push(json!({
            "source": package.utoc,
            "manifest": iostore_ops::manifest_value(aes_key, &package.utoc)?,
        }));
    }
    serde_json::to_string_pretty(&manifests)
        .map_err(|e| format!("Failed to serialize manifest JSON: {e}"))
}

fn archive_payload_root(root: &Path) -> std::path::PathBuf {
    let Ok(mut entries) = fs::read_dir(root) else {
        return root.to_path_buf();
    };
    let mut dirs = Vec::new();
    let mut files = 0usize;
    while let Some(Ok(entry)) = entries.next() {
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

fn source_kind(source: &PackageSource) -> &'static str {
    match source {
        PackageSource::IoStore(_) => "iostore",
        PackageSource::LegacyPak(_) => "legacy pak",
        PackageSource::RawDirectory(_) => "raw directory",
        PackageSource::Archive(_) => "archive",
        PackageSource::DirectoryPackages { .. } => "package directory",
    }
}
