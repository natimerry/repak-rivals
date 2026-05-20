use crate::install_mod::install_mod_logic::pak_files::repak_dir;
use crate::install_mod::install_mod_logic::patch_meshes;
use crate::install_mod::{InstallableMod, AES_KEY};
use crate::utils::collect_files;
use path_slash::PathExt;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use repak::Version;
use retoc::*;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tracing::{debug, info, instrument};
use walkdir::WalkDir;

const MOD_NAME_SUFFIX: &str = "_9999999_P";

struct TracingRetocLogProvider {
    installed_assets: Arc<AtomicI32>,
    base_progress: i32,
    mod_assets: i32,
    max_position: AtomicI32,
}

impl retoc::LogProvider for TracingRetocLogProvider {
    fn log(&self, msg: &str) {
        if msg.starts_with("[MaterialTags]") {
            debug!(target: "retoc", "{}", msg);
        } else {
            info!(target: "retoc", "{}", msg);
        }
    }

    fn progress(&self, position: u64, _length: u64) {
        let position = position.min(self.mod_assets.max(0) as u64) as i32;
        let previous_max = self.max_position.fetch_max(position, Ordering::SeqCst);
        let position = previous_max.max(position);
        let progress = self.base_progress.saturating_add(position);
        self.installed_assets.fetch_max(progress, Ordering::SeqCst);
    }
}

fn ensure_mod_name_suffix(name: &str) -> String {
    if name.ends_with(MOD_NAME_SUFFIX) {
        name.to_string()
    } else {
        format!("{name}{MOD_NAME_SUFFIX}")
    }
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

fn build_to_legacy_filter(utoc_path: PathBuf, config: Arc<retoc::Config>) -> Vec<String> {
    action_manifest(ActionManifest::new(utoc_path), config)
        .map(|ops| {
            let mut set = HashSet::new();

            for entry in &ops.oplog.entries {
                let package_name = entry.packagestoreentry.packagename.trim();
                if let Some(filter_path) = resolve_package_filter_path(package_name) {
                    set.insert(filter_path);
                }
            }

            set.into_iter().collect()
        })
        .unwrap_or_default()
}

fn to_legacy_config() -> Result<Arc<retoc::Config>, repak::Error> {
    let mut config = retoc::Config {
        container_header_version_override: None,
        ..Default::default()
    };
    let aes_toc =
        retoc::AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
            .map_err(|e| {
                repak::Error::Io(std::io::Error::other(format!(
                    "Failed to parse AES key: {e}"
                )))
            })?;
    config.aes_keys.insert(retoc::FGuid::default(), aes_toc);
    Ok(Arc::new(config))
}

fn hardlink_iostore_container(src_utoc: &Path, dst_dir: &Path) -> Result<(), repak::Error> {
    let stem = src_utoc
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            repak::Error::Io(std::io::Error::other(format!(
                "Invalid IoStore container filename: {}",
                src_utoc.display()
            )))
        })?;

    for ext in ["utoc", "ucas"] {
        let src = src_utoc.with_extension(ext);
        if !src.exists() {
            continue;
        }
        let dst = dst_dir.join(format!("{stem}.{ext}"));
        if let Err(link_error) = std::fs::hard_link(&src, &dst) {
            debug!(
                src = %src.display(),
                dst = %dst.display(),
                error = %link_error,
                "Falling back to copying IoStore container"
            );
            std::fs::copy(&src, &dst).map_err(|copy_error| {
                repak::Error::Io(std::io::Error::other(format!(
                    "Failed to hardlink or copy {} to {}: hardlink: {link_error}; copy: {copy_error}",
                    src.display(),
                    dst.display()
                )))
            })?;
        }
    }

    Ok(())
}

fn should_open_fast_game_container(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let stem_lower = stem.to_ascii_lowercase();
    stem_lower == "global"
        || (stem_lower.starts_with("pakchunk") && stem_lower.contains("character"))
}

fn prepare_fast_to_legacy_input(
    selected_utoc: &Path,
    mods_dir: &Path,
    game_paks_dir: &Path,
) -> Result<tempfile::TempDir, repak::Error> {
    let temp_dir = tempfile::tempdir_in(game_paks_dir).map_err(repak::Error::Io)?;
    let temp_path = temp_dir.path();

    debug!(mods_dir = %mods_dir.display(), selected_utoc = %selected_utoc.display(), "Preparing fast to-legacy input");
    hardlink_iostore_container(selected_utoc, temp_path)?;

    for entry in std::fs::read_dir(game_paks_dir).map_err(repak::Error::Io)? {
        let path = entry.map_err(repak::Error::Io)?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("utoc")
            && should_open_fast_game_container(&path)
        {
            hardlink_iostore_container(&path, temp_path)?;
        }
    }

    Ok(temp_dir)
}

fn collect_fast_to_legacy_inputs(
    mod_utocs: impl IntoIterator<Item = PathBuf>,
    game_paks_dir: &Path,
) -> Result<Vec<PathBuf>, repak::Error> {
    let mut inputs = Vec::new();
    let mut seen = HashSet::new();

    for path in mod_utocs {
        if seen.insert(path.clone()) {
            inputs.push(path);
        }
    }

    for entry in std::fs::read_dir(game_paks_dir).map_err(repak::Error::Io)? {
        let path = entry.map_err(repak::Error::Io)?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("utoc")
            && should_open_fast_game_container(&path)
            && seen.insert(path.clone())
        {
            inputs.push(path);
        }
    }

    Ok(inputs)
}

pub fn convert_directory_to_iostore(
    pak: &InstallableMod,
    mod_dir: PathBuf,
    to_pak_dir: PathBuf,
    packed_files_count: Arc<AtomicI32>,
    kawaii_physics_usmap: Option<PathBuf>,
    kawaii_porter: bool,
) -> Result<(), repak::Error> {
    let mod_type = pak.mod_type.clone();
    if mod_type == "Audio" || mod_type == "Movies" {
        debug!("{} mod detected. Not creating iostore packages", mod_type);
        repak_dir(pak, to_pak_dir, mod_dir, &packed_files_count)?;
        return Ok(());
    }

    let normalized_mod_name = ensure_mod_name_suffix(&pak.mod_name);

    let mut pak_name = normalized_mod_name.clone();
    pak_name.push_str(".pak");

    let mut utoc_name = normalized_mod_name;
    utoc_name.push_str(".utoc");

    let mut paths = vec![];
    collect_files(&mut paths, &to_pak_dir)?;

    if pak.fix_mesh {
        patch_meshes::mesh_patch(&mut paths, &to_pak_dir.to_path_buf())?;
    }

    let mut action = ActionToZen::new(
        to_pak_dir.clone(),
        mod_dir.join(utoc_name),
        EngineVersion::UE5_3,
        Some(compression::CompressionMethod::Oodle),
    )
    .with_obfuscation(pak.obfuscated);

    if kawaii_porter {
        let usmap = kawaii_physics_usmap.clone().ok_or_else(|| {
            repak::Error::Io(std::io::Error::other(
                "KawaiiPhysics porting is enabled, but no USMAP file was provided",
            ))
        })?;
        debug!(
            usmap = %usmap.display(),
            "Passing USMAP to KawaiiPhysics porter"
        );
        action = action.with_kawaii_physics_port(usmap);
    }

    let mut config = Config {
        container_header_version_override: None,
        port_kawaii_physics: kawaii_porter,
        kawaii_physics_usmap: kawaii_physics_usmap,
        kawaii_physics_force_rebuild: true,
        ..Default::default()
    };

    let aes_toc =
        retoc::AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
            .map_err(|e| {
                repak::Error::Io(std::io::Error::other(format!(
                    "Failed to parse AES key: {e}"
                )))
            })?;

    config.aes_keys.insert(FGuid::default(), aes_toc.clone());
    let config = Arc::new(config);

    let base_progress = packed_files_count.load(Ordering::SeqCst);
    retoc::set_log_provider(Arc::new(TracingRetocLogProvider {
        installed_assets: packed_files_count.clone(),
        base_progress,
        mod_assets: pak.total_files.max(1).min(i32::MAX as usize) as i32,
        max_position: AtomicI32::new(0),
    }));
    action_to_zen(action, config).map_err(|e| {
        repak::Error::Io(std::io::Error::other(format!(
            "Failed to convert to Zen: {e}"
        )))
    })?;

    // NOW WE CREATE THE FAKE PAK FILE WITH THE CONTENTS BEING A TEXT FILE LISTING ALL CHUNKNAMES

    let output_file = File::create(mod_dir.join(pak_name))?;

    let rel_paths = paths
        .par_iter()
        .map(|p| {
            let rel = p.strip_prefix(&to_pak_dir).map_err(|e| {
                repak::Error::Io(std::io::Error::other(format!(
                    "File is not in input directory: {} ({e})",
                    p.display()
                )))
            })?;
            let rel = rel.to_slash().ok_or_else(|| {
                repak::Error::Io(std::io::Error::other(format!(
                    "Failed to convert path to slash form: {}",
                    rel.display()
                )))
            })?;
            Ok(rel.to_string())
        })
        .collect::<Result<Vec<_>, repak::Error>>()?;

    let builder = repak::PakBuilder::new()
        .compression(vec![pak.compression])
        .key(AES_KEY.clone().0);

    let mut pak_writer = builder.writer(
        BufWriter::new(output_file),
        Version::V11,
        pak.mount_point.clone(),
        Some(pak.path_hash_seed.parse().map_err(|e| {
            repak::Error::Io(std::io::Error::other(format!(
                "Failed to parse path hash seed '{}': {e}",
                pak.path_hash_seed
            )))
        })?),
    );
    let entry_builder = pak_writer.entry_builder();

    let rel_paths_bytes: Vec<u8> = rel_paths.join("\n").into_bytes();
    let entry = entry_builder
        .build_entry(true, rel_paths_bytes, "chunknames")
        .map_err(|e| {
            repak::Error::Io(std::io::Error::other(format!(
                "Failed to build chunknames entry: {e}"
            )))
        })?;

    pak_writer.write_entry("chunknames".to_string(), entry)?;
    pak_writer.write_index()?;

    log::info!("Wrote pak file successfully");
    let minimum_progress =
        base_progress.saturating_add(pak.total_files.max(1).min(i32::MAX as usize) as i32);
    let current_progress = packed_files_count.load(Ordering::SeqCst);
    if current_progress < minimum_progress {
        packed_files_count.store(minimum_progress, Ordering::SeqCst);
    }
    Ok(())

    // now generate the fake pak file
}

#[instrument(skip_all)]
pub fn to_legacy_uasset(
    pak: PathBuf,
    output_dir: PathBuf,
    game_paks_dir: PathBuf,
    _packed_files_count: &AtomicI32,
) -> Result<(), repak::Error> {
    retoc::reset_log_provider_to_stdout();
    info!(
        pak = %pak.display(),
        output_dir = %output_dir.display(),
        game_paks_dir = %game_paks_dir.display(),
        "Starting to-legacy conversion"
    );

    let temp_dir = tempfile::tempdir().map_err(repak::Error::Io)?;
    let temp_path = temp_dir.path().to_path_buf();
    let mod_stem = pak
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            repak::Error::Io(std::io::Error::other(format!(
                "Invalid mod filename for to-legacy conversion: {}",
                pak.display()
            )))
        })?
        .to_string();

    // Copy mod files into the paks dir
    let mut copied_files = vec![];
    for ext in &["pak", "utoc", "ucas"] {
        let src = pak.with_extension(ext);
        let dst = game_paks_dir.join(format!("{}.{}", mod_stem, ext));
        if src.exists() {
            debug!(src = %src.display(), dst = %dst.display(), "Copying mod companion into game Paks");
            std::fs::copy(&src, &dst).map_err(repak::Error::Io)?;
            copied_files.push(dst);
        }
    }

    // Build filter list
    let utoc_path = pak.with_extension("utoc");
    let config = to_legacy_config()?;
    let filter = build_to_legacy_filter(utoc_path, config.clone());
    info!(package_count = filter.len(), "Prepared to-legacy filter");

    let legacy_output_dir = temp_path.join(mod_stem);
    std::fs::create_dir_all(&legacy_output_dir).map_err(repak::Error::Io)?;
    debug!(legacy_output_dir = %legacy_output_dir.display(), "Extracting legacy assets");

    let games_pak_dir_clone = game_paks_dir.clone();
    let legacy_output_dir_clone = legacy_output_dir.clone();

    let result = std::thread::spawn(move || {
        info!("retoc to-legacy started");
        retoc::action_to_legacy(
            ActionToLegacy {
                input: games_pak_dir_clone,
                output: legacy_output_dir_clone, // directory, not .pak
                filter,
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
    })
    .join()
    .map_err(|_| repak::Error::Io(std::io::Error::other("retoc to-legacy thread panicked")))?
    .map_err(|e| repak::Error::Io(std::io::Error::other(e.to_string())));

    // Copy extracted files from temp legacy dir to the actual output dir
    debug!("Copying converted files into final output...");
    for entry in WalkDir::new(&legacy_output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let src = entry.path();
        let relative = src.strip_prefix(&legacy_output_dir).map_err(|e| {
            repak::Error::Io(std::io::Error::other(format!(
                "Converted file is outside legacy output directory: {} ({e})",
                src.display()
            )))
        })?;
        let dst = output_dir.join(relative);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(repak::Error::Io)?;
        }
        std::fs::copy(src, &dst).map_err(repak::Error::Io)?;
        info!("Copied {:?} → {:?}", src, dst);
    }

    for dst in copied_files {
        debug!("Cleaning copied game file {}", dst.display());
        let _ = std::fs::remove_file(dst);
    }

    result?;
    info!(
        "Installing mod from {:#?} into {:#?}",
        &legacy_output_dir, &output_dir
    );
    return Ok(());
}

#[instrument(skip_all)]
pub fn to_legacy_uasset_fast(
    pak: PathBuf,
    output_dir: PathBuf,
    mods_dir: PathBuf,
    game_paks_dir: PathBuf,
) -> Result<(), repak::Error> {
    info!(
        pak = %pak.display(),
        output_dir = %output_dir.display(),
        mods_dir = %mods_dir.display(),
        game_paks_dir = %game_paks_dir.display(),
        "Starting fast to-legacy conversion"
    );

    let mod_stem = pak
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            repak::Error::Io(std::io::Error::other(format!(
                "Invalid mod filename for fast to-legacy conversion: {}",
                pak.display()
            )))
        })?
        .to_string();

    let config = to_legacy_config()?;
    let filter = build_to_legacy_filter(pak.with_extension("utoc"), config.clone());
    info!(
        package_count = filter.len(),
        "Prepared fast to-legacy filter"
    );

    let input_dir =
        prepare_fast_to_legacy_input(&pak.with_extension("utoc"), &mods_dir, &game_paks_dir)?;
    let legacy_output_dir = output_dir.join(mod_stem);
    std::fs::create_dir_all(&legacy_output_dir).map_err(repak::Error::Io)?;

    std::thread::spawn({
        let input_dir = input_dir.path().to_path_buf();
        let legacy_output_dir = legacy_output_dir.clone();
        move || {
            info!("retoc fast to-legacy started");
            retoc::action_to_legacy(
                ActionToLegacy {
                    input: input_dir,
                    output: legacy_output_dir,
                    filter,
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
        }
    })
    .join()
    .map_err(|_| {
        repak::Error::Io(std::io::Error::other(
            "retoc fast to-legacy thread panicked",
        ))
    })?
    .map_err(|e| repak::Error::Io(std::io::Error::other(e.to_string())))?;

    info!("fast to-legacy conversion complete");
    Ok(())
}

#[instrument(skip_all, fields(mod_count = paks.len(), output_dir = %output_dir.display(), game_paks_dir = %game_paks_dir.display()))]
pub fn to_legacy_uasset_fast_batch(
    paks: &[PathBuf],
    output_dir: PathBuf,
    game_paks_dir: PathBuf,
) -> Result<Vec<PathBuf>, repak::Error> {
    let config = to_legacy_config()?;
    let mut items = Vec::new();
    let mut extracted_dirs = Vec::new();

    for pak in paks {
        let mod_stem = pak
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| {
                repak::Error::Io(std::io::Error::other(format!(
                    "Invalid mod filename for batch to-legacy conversion: {}",
                    pak.display()
                )))
            })?
            .to_string();
        let output = output_dir.join(&mod_stem);
        std::fs::create_dir_all(&output).map_err(repak::Error::Io)?;
        let filter = build_to_legacy_filter(pak.with_extension("utoc"), config.clone());
        info!(
            mod_name = %mod_stem,
            package_count = filter.len(),
            "Prepared batch to-legacy filter"
        );
        items.push(retoc::ActionToLegacyBatchItem {
            output: output.clone(),
            filter,
        });
        extracted_dirs.push(output);
    }

    let inputs = collect_fast_to_legacy_inputs(
        paks.iter().map(|pak| pak.with_extension("utoc")),
        &game_paks_dir,
    )?;
    info!(
        container_count = inputs.len(),
        item_count = items.len(),
        "Starting retoc batch to-legacy"
    );

    retoc::action_to_legacy_batch(
        retoc::ActionToLegacyBatch {
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
    .map_err(|e| repak::Error::Io(std::io::Error::other(e.to_string())))?;

    Ok(extracted_dirs)
}
