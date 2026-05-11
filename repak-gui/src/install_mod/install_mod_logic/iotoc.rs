use crate::install_mod::install_mod_logic::pak_files::repak_dir;
use crate::install_mod::install_mod_logic::patch_meshes;
use crate::install_mod::{InstallableMod, AES_KEY};
use crate::utils::collect_files;
use path_slash::PathExt;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use repak::Version;
use retoc::*;
use walkdir::WalkDir;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use tracing::{debug, info, instrument};

const MOD_NAME_SUFFIX: &str = "_9999999_P";

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
    packed_files_count: &AtomicI32,
) -> Result<(), repak::Error> {
    let mod_type = pak.mod_type.clone();
    if mod_type == "Audio" || mod_type == "Movies" {
        debug!("{} mod detected. Not creating iostore packages", mod_type);
        repak_dir(pak, to_pak_dir, mod_dir, packed_files_count)?;
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
        ..Default::default()
    };

    let aes_toc =
        retoc::AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
            .unwrap();

    config.aes_keys.insert(FGuid::default(), aes_toc.clone());
    let config = Arc::new(config);

    action_to_zen(action, config).expect("Failed to convert to zen");

    // NOW WE CREATE THE FAKE PAK FILE WITH THE CONTENTS BEING A TEXT FILE LISTING ALL CHUNKNAMES

    let output_file = File::create(mod_dir.join(pak_name))?;

    let rel_paths = paths
        .par_iter()
        .map(|p| {
            let rel = &p
                .strip_prefix(to_pak_dir.clone())
                .expect("file not in input directory")
                .to_slash()
                .expect("failed to convert to slash path");
            rel.to_string()
        })
        .collect::<Vec<_>>();

    let builder = repak::PakBuilder::new()
        .compression(vec![pak.compression])
        .key(AES_KEY.clone().0);

    let mut pak_writer = builder.writer(
        BufWriter::new(output_file),
        Version::V11,
        pak.mount_point.clone(),
        Some(pak.path_hash_seed.parse().unwrap()),
    );
    let entry_builder = pak_writer.entry_builder();

    let rel_paths_bytes: Vec<u8> = rel_paths.join("\n").into_bytes();
    let entry = entry_builder
        .build_entry(true, rel_paths_bytes, "chunknames")
        .expect("Failed to build entry");

    pak_writer.write_entry("chunknames".to_string(), entry)?;
    pak_writer.write_index()?;

    log::info!("Wrote pak file successfully");
    packed_files_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
    let temp_dir = tempfile::tempdir().map_err(repak::Error::Io)?;
    let temp_path = temp_dir.path().to_path_buf();
    let mod_stem = pak.file_stem().unwrap().to_str().unwrap();

    // Copy mod files into the paks dir
    let mut copied_files = vec![];
    for ext in &["pak", "utoc", "ucas"] {
        let src = pak.with_extension(ext);
        let dst = game_paks_dir.join(format!("{}.{}", mod_stem, ext));
        if src.exists() {
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
    let aes_toc =
        retoc::AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
            .unwrap();
    config.aes_keys.insert(retoc::FGuid::default(), aes_toc);
    let config = Arc::new(config);

    use std::collections::HashSet;

    let filter: Vec<String> = action_manifest(action_mn, config.clone())
        .map(|ops| {
            let mut set = HashSet::new();

            ops.oplog.entries.iter().for_each(|entry| {
                if let Some(stem) = std::path::Path::new(&entry.packagestoreentry.packagename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                {
                    set.insert(stem.to_string());
                }
            });

            set.into_iter().collect()
        })
        .unwrap_or_default();

    let legacy_output_dir = temp_path.join(mod_stem);
    std::fs::create_dir_all(&legacy_output_dir).map_err(repak::Error::Io)?;

    let games_pak_dir_clone = game_paks_dir.clone();
    let legacy_output_dir_clone = legacy_output_dir.clone();

    let result = std::thread::spawn(move || {
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
    .unwrap()
    .map_err(|e| repak::Error::Io(std::io::Error::other(e.to_string())));

    // Copy extracted files from temp legacy dir to the actual output dir
    for entry in WalkDir::new(&legacy_output_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let src = entry.path();
        let relative = src.strip_prefix(&legacy_output_dir).unwrap();
        let dst = output_dir.join(relative);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(repak::Error::Io)?;
        }
        std::fs::copy(src, &dst).map_err(repak::Error::Io)?;
        info!("Copied {:?} → {:?}", src, dst);
    }

    for dst in copied_files {
        let _ = std::fs::remove_file(dst);
    }

    result?;
    info!(
        "Installing mod from {:#?} into {:#?}",
        &legacy_output_dir, &output_dir
    );
    return Ok(());
}
