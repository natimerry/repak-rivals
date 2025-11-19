use crate::install_mod::install_mod_logic::patch_meshes::mesh_patch;
use crate::install_mod::{InstallableMod, AES_KEY};
use crate::utils::collect_files;
use log::debug;
use path_clean::PathClean;
use path_slash::PathExt;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use repak::Version;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicI32;
use tempfile::tempdir;

use super::iotoc::convert_directory_to_iostore;

pub fn extract_pak_to_dir(pak: &InstallableMod, install_dir: PathBuf) -> Result<(), repak::Error> {
    let pak_reader = pak.clone().reader.clone().unwrap();

    let mount_point = PathBuf::from(pak_reader.mount_point());
    let prefix = Path::new("../../../");

    struct UnpakEntry {
        entry_path: String,
        out_path: PathBuf,
        out_dir: PathBuf,
    }

    let entries = pak_reader
        .files()
        .into_iter()
        .map(|entry| {
            let full_path = mount_point.join(&entry);
            let out_path =
                install_dir
                    .join(full_path.strip_prefix(prefix).map_err(|_| {
                        repak::Error::PrefixMismatch {
                            path: full_path.to_string_lossy().to_string(),
                            prefix: prefix.to_string_lossy().to_string(),
                        }
                    })?)
                    .clean();

            if !out_path.starts_with(&install_dir) {
                return Err(repak::Error::WriteOutsideOutput(
                    out_path.to_string_lossy().to_string(),
                ));
            }

            let out_dir = out_path.parent().expect("will be a file").to_path_buf();

            Ok(Some(UnpakEntry {
                entry_path: entry.to_string(),
                out_path,
                out_dir,
            }))
        })
        .filter_map(|x| x.transpose())
        .collect::<Result<Vec<_>, _>>()?;

    entries.par_iter().for_each(|entry| {
        log::debug!("Unpacking: {}", entry.entry_path);
        fs::create_dir_all(&entry.out_dir).unwrap();
        let mut reader = BufReader::new(File::open(&pak.mod_path).unwrap());
        let buffer = pak_reader
            .get(&entry.entry_path, &mut reader)
            .expect("Failed to read entry");
        File::create(&entry.out_path)
            .unwrap()
            .write_all(&buffer)
            .unwrap();
        log::info!("Unpacked: {:?}", entry.out_path);
    });
    Ok(())
}


pub fn create_repak_from_pak(
    pak: &InstallableMod,
    mod_dir: PathBuf,
    packed_files_count: &AtomicI32,
) -> Result<(), repak::Error> {
    // extract the pak first into a temporary dir
    let temp_dir = tempdir().map_err(repak::Error::Io)?;
    let temp_path = temp_dir.path(); // Get the path of the temporary directory

    extract_pak_to_dir(pak, temp_path.to_path_buf())?;
    convert_directory_to_iostore(
        pak,
        mod_dir.clone(),
        temp_path.to_path_buf(),
        packed_files_count,
    )?;
    // repak_dir(pak, PathBuf::from(temp_path), mod_dir,packed_files_count)?;
    Ok(())
}

// leaving this here for legacy reasons
pub fn repak_dir(
    pak: &InstallableMod,
    to_pak_dir: PathBuf,
    mod_dir: PathBuf,
    installed_mods_ptr: &AtomicI32,
) -> Result<(), repak::Error> {
    let mut pak_name = pak.mod_name.clone();
    pak_name.push_str(".pak");
    let output_file = File::create(mod_dir.join(pak_name))?;

    let mut paths = vec![];
    collect_files(&mut paths, &to_pak_dir)?;

    if pak.fix_mesh {
        mesh_patch(&mut paths, &to_pak_dir.to_path_buf())?;
    }

    paths.sort();

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

    let partial_entry = paths
        .par_iter()
        .map(|p| {
            let rel = &p
                .strip_prefix(to_pak_dir.clone())
                .expect("file not in input directory")
                .to_slash()
                .expect("failed to convert to slash path");

            let entry = entry_builder
                .build_entry(true, std::fs::read(p).expect("WTF"), rel)
                .expect("Failed to build entry");
            (rel.to_string(), entry)
        })
        .collect::<Vec<_>>();

    let mut rel_paths = vec![];
    for (path, entry) in partial_entry {
        debug!("Writing: {}", path);
        pak_writer.write_entry(path.clone(), entry)?;
        installed_mods_ptr.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        rel_paths.push(path);
    }

    let rel_paths_bytes: Vec<u8> = rel_paths.join("\n").into_bytes();

    let entry = entry_builder
        .build_entry(true, rel_paths_bytes, "chunknames")
        .expect("Failed to build entry");

    pak_writer.write_entry("chunknames".to_string(), entry)?;
    pak_writer.write_index()?;

    log::info!("Wrote pak file successfully");
    Ok::<(), repak::Error>(())
}