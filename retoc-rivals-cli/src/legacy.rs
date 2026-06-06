use crate::cli::{FixKawaiiPhysicsArgs, PackArgs};
use crate::config::{read_saved_state, retoc_config};
use crate::kawaii_utils;
use crate::pack::pack;
use retoc::{
    action_manifest, action_to_legacy_batch, ActionManifest, ActionToLegacyBatch,
    ActionToLegacyBatchItem, Config,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn fix_kawaii_physics(
    aes_key: retoc::AesKey,
    args: FixKawaiiPhysicsArgs,
) -> Result<(), String> {
    if let Some(input) = args.input.as_deref() {
        return fix_kawaii_physics_directory(
            input,
            args.usmap.as_deref(),
            args.patch_default_hidden_mats,
            default_hidden_material_bitmaps(&args),
        );
    }

    let state = read_saved_state()?;
    let mods_dir = state.game_path;
    let game_paks_dir = state
        .game_chunk_path
        .ok_or_else(|| "No game Paks directory found in saved state".to_string())?;
    let usmap = kawaii_utils::resolve_kawaii_usmap(
        args.usmap
            .as_deref()
            .or(state.kawaii_physics_usmap.as_deref()),
    )?;

    let paks = installed_iostore_paks(&mods_dir)
        .map_err(|e| format!("Failed to scan {}: {e}", mods_dir.display()))?;
    if paks.is_empty() {
        return Err(format!(
            "No installed IoStore mods found in {}",
            mods_dir.display()
        ));
    }

    tracing::info!(mod_count = paks.len(), "Starting KawaiiPhysics repair");
    println!("Found {} installed IoStore mods", paks.len());
    let extracted_temp =
        tempfile::tempdir().map_err(|e| format!("Failed to create temp directory: {e}"))?;
    let extracted_dirs = to_legacy_uasset_fast_batch(
        aes_key.clone(),
        &paks,
        extracted_temp.path(),
        &game_paks_dir,
    )?;
    fs::create_dir_all(&args.output)
        .map_err(|e| format!("Failed to create {}: {e}", args.output.display()))?;

    for (idx, extracted_dir) in extracted_dirs.iter().enumerate() {
        println!(
            "[{}/{}] Rebuilding {}",
            idx + 1,
            extracted_dirs.len(),
            extracted_dir.display()
        );
        pack(
            aes_key.clone(),
            PackArgs {
                input: vec![extracted_dir.clone()],
                output: Some(args.output.clone()),
                separate_output_dirs: false,
                mount_point: "../../../".to_string(),
                path_hash_seed: "00000000".to_string(),
                no_mod_suffix: false,
                obfuscate: false,
                compression: crate::cli::CompressionArg::Oodle,
                kawaii_physics: true,
                kawaii_physics_usmap: Some(usmap.clone()),
                patch_default_hidden_mats: args.patch_default_hidden_mats,
                default_hidden_material_bitmaps: args.default_hidden_material_bitmaps.clone(),
                game_paks_dir: Some(game_paks_dir.clone()),
                full_iostore_check: false,
            },
        )?;
    }

    println!("Wrote fixed mods to {}", args.output.display());
    Ok(())
}

fn fix_kawaii_physics_directory(
    input: &Path,
    usmap_arg: Option<&Path>,
    patch_default_hidden_mats: bool,
    default_hidden_material_bitmaps: Option<&[u64]>,
) -> Result<(), String> {
    if !input.is_dir() {
        return Err(format!("Input is not a directory: {}", input.display()));
    }

    let saved_usmap = if usmap_arg.is_none() {
        read_saved_state()
            .ok()
            .and_then(|state| state.kawaii_physics_usmap)
    } else {
        None
    };
    let usmap = kawaii_utils::resolve_kawaii_usmap(usmap_arg.or(saved_usmap.as_deref()))?;

    tracing::info!(input = %input.display(), usmap = %usmap.display(), "Porting KawaiiPhysics assets in-place");
    println!("Fixing KawaiiPhysics assets in {}", input.display());
    let ported = retoc::port_kawaii_physics_directory(
        input,
        &usmap,
        true,
        patch_default_hidden_mats,
        default_hidden_material_bitmaps,
        None,
    )
    .map_err(|e| format!("KawaiiPhysics directory fix failed: {e:#}"))?;
    println!("Ported {ported} KawaiiPhysics anim nodes");
    Ok(())
}

fn default_hidden_material_bitmaps(args: &FixKawaiiPhysicsArgs) -> Option<&[u64]> {
    if !args.default_hidden_material_bitmaps.is_empty() {
        Some(args.default_hidden_material_bitmaps.as_slice())
    } else {
        None
    }
}

fn installed_iostore_paks(mods_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paks = Vec::new();
    for entry in fs::read_dir(mods_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("utoc") {
            continue;
        }
        let pak = path.with_extension("pak");
        let ucas = path.with_extension("ucas");
        if pak.exists() && ucas.exists() {
            paks.push(pak);
        }
    }
    paks.sort();
    Ok(paks)
}

fn to_legacy_uasset_fast_batch(
    aes_key: retoc::AesKey,
    paks: &[PathBuf],
    output_dir: &Path,
    game_paks_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let config = retoc_config(aes_key);
    let mut items = Vec::new();
    let mut extracted_dirs = Vec::new();

    for pak in paks {
        let stem = pak
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("Invalid mod filename: {}", pak.display()))?
            .to_string();
        let output = output_dir.join(&stem);
        fs::create_dir_all(&output)
            .map_err(|e| format!("Failed to create {}: {e}", output.display()))?;
        let filter = build_to_legacy_filter(pak.with_extension("utoc"), config.clone());
        items.push(ActionToLegacyBatchItem {
            inputs: vec![pak.with_extension("utoc")],
            output: output.clone(),
            filter,
        });
        extracted_dirs.push(output);
    }

    let inputs = collect_fast_to_legacy_inputs(
        paks.iter().map(|pak| pak.with_extension("utoc")),
        game_paks_dir,
    )?;
    action_to_legacy_batch(
        ActionToLegacyBatch {
            inputs,
            items,
            no_assets: false,
            no_shaders: false,
            no_compres_shaders: true,
            dry_run: false,
            version: None,
            verbose: true,
            debug: false,
            no_parallel: false,
        },
        config,
    )
    .map_err(|e| format!("Batch to-legacy extraction failed: {e}"))?;

    Ok(extracted_dirs)
}

fn build_to_legacy_filter(utoc_path: PathBuf, config: Arc<Config>) -> Vec<String> {
    action_manifest(ActionManifest::new(utoc_path), config)
        .map(|ops| {
            let mut set = HashSet::new();
            for entry in &ops.oplog.entries {
                let Some(package_data) = entry.packagedata.first() else {
                    continue;
                };
                if let Some(filter_path) = resolve_package_filter_path(package_data.filename.trim())
                {
                    set.insert(filter_path);
                }
            }
            set.into_iter().collect()
        })
        .unwrap_or_default()
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

fn collect_fast_to_legacy_inputs(
    mod_utocs: impl IntoIterator<Item = PathBuf>,
    game_paks_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut inputs = Vec::new();
    let mut seen = HashSet::new();

    for path in mod_utocs {
        if seen.insert(path.clone()) {
            inputs.push(path);
        }
    }
    for entry in fs::read_dir(game_paks_dir)
        .map_err(|e| format!("Failed to scan {}: {e}", game_paks_dir.display()))?
    {
        let path = entry
            .map_err(|e| format!("Failed to read {}: {e}", game_paks_dir.display()))?
            .path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("utoc")
            && should_open_fast_game_container(&path)
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
