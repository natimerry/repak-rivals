use crate::install_mod::install_mod_logic::pak_files::repak_dir;
use crate::install_mod::install_mod_logic::patch_meshes;
use crate::install_mod::{InstallableMod, AES_KEY};
use crate::utils::collect_files;
use path_slash::PathExt;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use repak::Version;
use retoc::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
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

pub fn convert_directory_to_iostore(
    pak: &InstallableMod,
    mod_dir: PathBuf,
    to_pak_dir: PathBuf,
    packed_files_count: Arc<AtomicI32>,
    kawaii_physics_usmap: Option<PathBuf>,
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

    let action = ActionToZen::new(
        to_pak_dir.clone(),
        mod_dir.join(utoc_name),
        EngineVersion::UE5_3,
        Some(compression::CompressionMethod::Oodle),
    )
    .with_obfuscation(pak.obfuscated);

    let mut config = Config {
        container_header_version_override: None,
        port_kawaii_physics: true,
        kawaii_physics_usmap: kawaii_physics_usmap,
        kawaii_physics_force_rebuild: true,
        ..Default::default()
    };

    let aes_toc = retoc::AesKey::from_str(
        "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
    )
    .map_err(|e| repak::Error::Io(std::io::Error::other(format!("Failed to parse AES key: {e}"))))?;

    config.aes_keys.insert(FGuid::default(), aes_toc.clone());
    let config = Arc::new(config);

    let base_progress = packed_files_count.load(Ordering::SeqCst);
    retoc::set_log_provider(Arc::new(TracingRetocLogProvider {
        installed_assets: packed_files_count.clone(),
        base_progress,
        mod_assets: pak.total_files.max(1).min(i32::MAX as usize) as i32,
        max_position: AtomicI32::new(0),
    }));
    action_to_zen(action, config)
        .map_err(|e| repak::Error::Io(std::io::Error::other(format!("Failed to convert to Zen: {e}"))))?;

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
        .map_err(|e| repak::Error::Io(std::io::Error::other(format!("Failed to build chunknames entry: {e}"))))?;

    pak_writer.write_entry("chunknames".to_string(), entry)?;
    pak_writer.write_index()?;

    log::info!("Wrote pak file successfully");
    let minimum_progress = base_progress
        .saturating_add(pak.total_files.max(1).min(i32::MAX as usize) as i32);
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
    println!("Starting to-legacy conversion for {}", pak.display());
    println!("Output directory: {}", output_dir.display());
    println!("Temporary game Paks directory: {}", game_paks_dir.display());

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
            println!("Copying {} to {}", src.display(), dst.display());
            std::fs::copy(&src, &dst).map_err(repak::Error::Io)?;
            copied_files.push(dst);
        }
    }

    // Build filter list
    let utoc_path = pak.with_extension("utoc");
    let action_mn = ActionManifest::new(utoc_path.clone());
    let mut config = retoc::Config {
        container_header_version_override: None,
        ..Default::default()
    };
    let aes_toc = retoc::AesKey::from_str(
        "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
    )
    .map_err(|e| repak::Error::Io(std::io::Error::other(format!("Failed to parse AES key: {e}"))))?;
    config.aes_keys.insert(retoc::FGuid::default(), aes_toc);
    let config = Arc::new(config);

    use std::collections::HashSet;

    let filter: Vec<String> = action_manifest(action_mn, config.clone())
        .map(|ops| {
            let mut set = HashSet::new();

            ops.oplog.entries.iter().for_each(|entry| {
                let package_name = entry.packagestoreentry.packagename.trim();
                if !package_name.is_empty() {
                    set.insert(package_name.replace('\\', "/"));
                }
            });

            set.into_iter().collect()
        })
        .unwrap_or_default();
    println!("Prepared to-legacy filter with {} packages", filter.len());

    let legacy_output_dir = temp_path.join(mod_stem);
    std::fs::create_dir_all(&legacy_output_dir).map_err(repak::Error::Io)?;
    println!("Extracting legacy assets into {}", legacy_output_dir.display());

    let games_pak_dir_clone = game_paks_dir.clone();
    let legacy_output_dir_clone = legacy_output_dir.clone();

    let result = std::thread::spawn(move || {
        println!("retoc to-legacy started. This can take several minutes for large mods.");
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
    println!("Copying converted files into final output...");
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
        println!("Cleaning copied game file {}", dst.display());
        let _ = std::fs::remove_file(dst);
    }

    result?;
    println!("to-legacy conversion complete.");
    info!(
        "Installing mod from {:#?} into {:#?}",
        &legacy_output_dir, &output_dir
    );
    return Ok(());
}
