use crate::config::{read_saved_state, retoc_config};
use crate::source::IoStorePackage;
use crate::util::ensure_mod_name_suffix;
use retoc::{action_manifest, action_to_legacy_batch, ActionManifest, ActionToLegacyBatch};
use retoc::{ActionToLegacyBatchItem, Config};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub struct ExtractedPackage {
    pub source: PathBuf,
    pub output: PathBuf,
}

pub fn manifest_filter(aes_key: &retoc::AesKey, utoc: &Path) -> Result<Vec<String>, String> {
    build_to_legacy_filter(utoc.to_path_buf(), retoc_config(aes_key.clone()))
}

pub fn manifest_value(aes_key: &retoc::AesKey, utoc: &Path) -> Result<serde_json::Value, String> {
    let manifest = action_manifest(
        ActionManifest::new(utoc.to_path_buf()),
        retoc_config(aes_key.clone()),
    )
    .map_err(|e| format!("Manifest failed for {}: {e}", utoc.display()))?;
    serde_json::to_value(manifest)
        .map_err(|e| format!("Failed to serialize manifest for {}: {e}", utoc.display()))
}

pub fn to_legacy_single(
    aes_key: &retoc::AesKey,
    package: &IoStorePackage,
    output: &Path,
    filters: &[String],
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
    verbose: bool,
) -> Result<ExtractedPackage, String> {
    let mut extracted = to_legacy_outputs(
        aes_key,
        std::slice::from_ref(package),
        vec![output.to_path_buf()],
        filters,
        game_paks_dir,
        full_iostore_check,
        verbose,
    )?;
    extracted
        .pop()
        .ok_or_else(|| "IoStore extraction produced no output".to_string())
}

pub fn to_legacy_prefixed(
    aes_key: &retoc::AesKey,
    packages: &[IoStorePackage],
    output_prefix: &str,
    filters: &[String],
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
    verbose: bool,
) -> Result<Vec<ExtractedPackage>, String> {
    let outputs = packages
        .iter()
        .map(|package| {
            package
                .utoc
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(format!("{}{}", output_prefix, package.stem()))
        })
        .collect();

    to_legacy_outputs(
        aes_key,
        packages,
        outputs,
        filters,
        game_paks_dir,
        full_iostore_check,
        verbose,
    )
}

pub fn to_legacy_under_root(
    aes_key: &retoc::AesKey,
    packages: &[IoStorePackage],
    output_root: &Path,
    filters: &[String],
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
    verbose: bool,
) -> Result<Vec<ExtractedPackage>, String> {
    let outputs = packages
        .iter()
        .map(|package| output_root.join(package.stem()))
        .collect();

    to_legacy_outputs(
        aes_key,
        packages,
        outputs,
        filters,
        game_paks_dir,
        full_iostore_check,
        verbose,
    )
}

pub fn to_legacy_outputs(
    aes_key: &retoc::AesKey,
    packages: &[IoStorePackage],
    outputs: Vec<PathBuf>,
    filters: &[String],
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
    verbose: bool,
) -> Result<Vec<ExtractedPackage>, String> {
    if packages.len() != outputs.len() {
        return Err("IoStore package/output count mismatch".to_string());
    }
    if packages.is_empty() {
        return Ok(Vec::new());
    }

    let config = retoc_config(aes_key.clone());
    let mut items = Vec::with_capacity(packages.len());
    let mut extracted = Vec::with_capacity(packages.len());
    for (package, output) in packages.iter().zip(outputs) {
        let item_filter = if filters.is_empty() {
            build_to_legacy_filter(package.utoc.clone(), config.clone())?
        } else {
            filters.to_vec()
        };
        fs::create_dir_all(&output)
            .map_err(|e| format!("Failed to create {}: {e}", output.display()))?;
        items.push(ActionToLegacyBatchItem {
            inputs: vec![package.utoc.clone()],
            output: output.clone(),
            filter: item_filter,
        });
        extracted.push(ExtractedPackage {
            source: package.utoc.clone(),
            output,
        });
    }

    let selected = packages.iter().map(|package| package.utoc.clone());
    let inputs = collect_to_legacy_inputs(selected, game_paks_dir, full_iostore_check)?;
    tracing::info!(
        container_count = inputs.len(),
        item_count = items.len(),
        full_iostore_check,
        "Running retoc batch to-legacy"
    );
    action_to_legacy_batch(
        ActionToLegacyBatch {
            inputs,
            items,
            no_assets: false,
            no_shaders: false,
            no_compres_shaders: true,
            dry_run: false,
            version: None,
            verbose,
            debug: false,
            no_parallel: false,
        },
        config,
    )
    .map_err(|e| format!("IoStore to-legacy extraction failed: {e}"))?;

    Ok(extracted)
}

pub fn copy_iostore_package(
    package: &IoStorePackage,
    output_dir: &Path,
    no_mod_suffix: bool,
) -> Result<PathBuf, String> {
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create {}: {e}", output_dir.display()))?;
    let name = if no_mod_suffix {
        package.stem()
    } else {
        ensure_mod_name_suffix(&package.stem())
    };

    for (src, ext) in [
        (&package.pak, "pak"),
        (&package.utoc, "utoc"),
        (&package.ucas, "ucas"),
    ] {
        let dst = output_dir.join(format!("{name}.{ext}"));
        if same_file(src, &dst) {
            continue;
        }
        fs::copy(src, &dst)
            .map_err(|e| format!("Failed to copy {} to {}: {e}", src.display(), dst.display()))?;
    }

    Ok(output_dir.join(format!("{name}.utoc")))
}

pub fn resolve_game_paks_dir(arg: &Option<PathBuf>) -> Result<Option<PathBuf>, String> {
    if let Some(path) = arg {
        return Ok(Some(path.clone()));
    }

    match read_saved_state() {
        Ok(state) => Ok(state.game_chunk_path),
        Err(_) => Ok(None),
    }
}

fn build_to_legacy_filter(utoc_path: PathBuf, config: Arc<Config>) -> Result<Vec<String>, String> {
    let manifest = action_manifest(ActionManifest::new(utoc_path.clone()), config)
        .map_err(|e| format!("Failed to build filter for {}: {e}", utoc_path.display()))?;
    let mut set = HashSet::new();

    for entry in &manifest.oplog.entries {
        let Some(package_data) = entry.packagedata.first() else {
            continue;
        };
        if let Some(filter_path) = resolve_package_filter_path(package_data.filename.trim()) {
            set.insert(filter_path);
        }
    }

    let mut filters = set.into_iter().collect::<Vec<_>>();
    filters.sort();
    Ok(filters)
}

fn resolve_package_filter_path(package_name: &str) -> Option<String> {
    let package_name = package_name.trim().replace('\\', "/");
    if package_name.is_empty() {
        return None;
    }
    if package_name.starts_with("../../../") {
        return Some(package_name);
    }
    if let Some(rest) = package_name.strip_prefix("/Game/") {
        return Some(format!("../../../Marvel/Content/{rest}"));
    }
    if package_name == "/Game" {
        return Some("../../../Marvel/Content".to_string());
    }
    if let Some(rest) = package_name.strip_prefix("/Engine/") {
        return Some(format!("../../../Engine/Content/{rest}"));
    }
    if package_name == "/Engine" {
        return Some("../../../Engine/Content".to_string());
    }
    Some(package_name.trim_start_matches('/').to_string())
}

fn collect_to_legacy_inputs(
    selected_utocs: impl IntoIterator<Item = PathBuf>,
    game_paks_dir: Option<&Path>,
    full_iostore_check: bool,
) -> Result<Vec<PathBuf>, String> {
    let mut inputs = Vec::new();
    let mut seen = HashSet::new();

    for path in selected_utocs {
        if seen.insert(path.clone()) {
            inputs.push(path);
        }
    }

    if full_iostore_check && game_paks_dir.is_none() {
        return Err(
            "--full-iostore-check requires --game-paks-dir or saved GUI config".to_string(),
        );
    }

    let Some(game_paks_dir) = game_paks_dir else {
        return Ok(inputs);
    };

    for entry in fs::read_dir(game_paks_dir)
        .map_err(|e| format!("Failed to scan {}: {e}", game_paks_dir.display()))?
    {
        let path = entry
            .map_err(|e| format!("Failed to read {}: {e}", game_paks_dir.display()))?
            .path();
        let is_utoc = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("utoc"));
        if is_utoc
            && (full_iostore_check || should_open_fast_game_container(&path))
            && seen.insert(path.clone())
        {
            inputs.push(path);
        }
    }

    Ok(inputs)
}

fn should_open_fast_game_container(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let stem = stem.to_ascii_lowercase();
    stem == "global" || (stem.starts_with("pakchunk") && stem.contains("character"))
}

fn same_file(left: &Path, right: &Path) -> bool {
    let Ok(left) = left.canonicalize() else {
        return false;
    };
    let Ok(right) = right.canonicalize() else {
        return false;
    };
    left == right
}
