use crate::install_mod::install_mod_logic::pak_files::repak_dir;
use crate::install_mod::install_mod_logic::patch_meshes;
use crate::install_mod::{InstallableMod, AES_KEY};
use crate::utils::collect_files;
use log::debug;
use path_slash::PathExt;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use repak::Version;
use retoc::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;

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

pub fn repack_iostore_mod(
    pak: &InstallableMod,
    mod_dir: PathBuf,
    game_paks_dir: &Path, // e.g. "C:\...\Marvel\Content\Paks"
    packed_files_count: &AtomicI32,
) -> Result<(), repak::Error> {
    let temp_dir = tempfile::tempdir().map_err(repak::Error::Io)?;
    let temp_path = temp_dir.path().to_path_buf();

    let mut config = retoc::Config {
        container_header_version_override: None,
        ..Default::default()
    };
    let aes_toc =
        retoc::AesKey::from_str("0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74")
            .unwrap();
    config.aes_keys.insert(retoc::FGuid::default(), aes_toc);
    let config = Arc::new(config);

    retoc::action_to_legacy(
        ActionToLegacy {
            input: game_paks_dir.to_path_buf(), // <-- paks dir, not mod utoc
            output: temp_path.clone(),
            filter: vec![pak
                .mod_path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()], // filter to only this mod's assets
            no_assets: false,
            no_shaders: true,
            no_compres_shaders: true,
            dry_run: false,
            version: None,
            verbose: false,
            debug: false,
            no_parallel: false,
        },
        config,
    )
    .map_err(|e| repak::Error::Io(std::io::Error::other(e.to_string())))?;

    convert_directory_to_iostore(pak, mod_dir, temp_path, packed_files_count)
}
