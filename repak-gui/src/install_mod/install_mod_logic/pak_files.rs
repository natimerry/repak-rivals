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
use std::io::{BufReader, BufWriter, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tempfile::tempdir;

use super::{ensure_mod_name_suffix, iotoc::convert_directory_to_iostore};

/// Wwise content lives here; nothing else may be written into a stripped pak.
const STRIPPABLE_ROOT: &str = "marvel/content/wwiseaudio/";
const STRIPPABLE_EXTENSIONS: [&str; 2] = ["bnk", "wem"];

/// Whether `entry` (relative to the pak's `mount_point`) is a Wwise bank or media file under
/// `Marvel/Content/WwiseAudio/`. Stripped paks hide their file list, so Repak only writes
/// one for content that can't be anything else.
pub fn is_strippable_audio_entry(mount_point: &str, entry: &str) -> bool {
    let entry = entry.replace('\\', "/");
    let mut mount = mount_point.replace('\\', "/");
    if entry.contains(':') || mount.contains(':') || entry.split('/').any(|part| part == "..") {
        return false;
    }
    if !mount.ends_with('/') {
        mount.push('/');
    }
    // The mount point is the only place a leading "../" is allowed.
    let mut mount = mount.as_str();
    while let Some(rest) = mount.strip_prefix("../") {
        mount = rest;
    }
    let full = format!("{mount}/{entry}");
    let parts = full
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    if parts.contains(&"..") {
        return false;
    }
    let Some(name) = parts.last() else {
        return false;
    };
    parts
        .join("/")
        .to_ascii_lowercase()
        .starts_with(STRIPPABLE_ROOT)
        && Path::new(name)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                STRIPPABLE_EXTENSIONS
                    .iter()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed))
            })
}

/// Entries that stop a pak from being stripped. Entries Repak drops from every pak anyway
/// (`chunknames`, `patched_files`, `desktop.ini`) don't count.
pub fn strip_blockers<'a>(
    mount_point: &str,
    entries: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    entries
        .into_iter()
        .filter(|entry| !crate::install_mod::is_unsupported_companion_entry(mount_point, entry))
        .filter(|entry| !is_strippable_audio_entry(mount_point, entry))
        .map(str::to_string)
        .collect()
}

pub fn summarize_strip_blockers(blockers: &[String]) -> String {
    let shown = blockers
        .iter()
        .take(3)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match blockers.len().saturating_sub(3) {
        0 => shown,
        more => format!("{shown} and {more} more"),
    }
}

pub fn pak_contains_unsupported_entries(path: &Path) -> Result<bool, repak::Error> {
    let reader = repak::PakBuilder::new()
        .key(AES_KEY.clone().0)
        .reader(&mut BufReader::new(File::open(path)?))?;
    Ok(reader.files().iter().any(|entry| {
        crate::install_mod::is_unsupported_companion_entry(reader.mount_point(), entry)
    }))
}

pub fn rewrite_unsupported_companion_pak(path: &Path) -> Result<bool, repak::Error> {
    let reader = repak::PakBuilder::new()
        .key(AES_KEY.clone().0)
        .reader(&mut BufReader::new(File::open(path)?))?;
    let unsupported_entries = reader
        .files()
        .into_iter()
        .filter(|entry| {
            crate::install_mod::is_unsupported_companion_entry(reader.mount_point(), entry)
        })
        .collect::<Vec<_>>();
    if unsupported_entries.is_empty() {
        return Ok(false);
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut replacement = tempfile::NamedTempFile::new_in(parent).map_err(repak::Error::Io)?;
    std::io::copy(&mut File::open(path)?, replacement.as_file_mut())?;

    let mut writer = reader.into_pakwriter(replacement.as_file_mut())?;
    for entry in unsupported_entries {
        writer.remove_entry(&entry);
    }
    let replacement_file = writer.write_index()?;
    let replacement_len = replacement_file.stream_position()?;
    replacement_file.set_len(replacement_len)?;
    replacement_file.sync_all()?;
    replacement
        .persist(path)
        .map_err(|error| repak::Error::Io(error.error))?;
    Ok(true)
}

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
        .filter(|entry| {
            !crate::install_mod::is_unsupported_companion_entry(pak_reader.mount_point(), entry)
        })
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
        // log::info!("Unpacked: {:?}", entry.out_path);
    });
    Ok(())
}

pub fn create_repak_from_pak(
    pak: &InstallableMod,
    mod_dir: PathBuf,
    packed_files_count: Arc<AtomicI32>,
    kawaii_physics_usmap: &Option<PathBuf>,
    kawaii_porter: bool,
    patch_default_hidden_materials: bool,
    default_hidden_material_bitmaps: Option<&[u64]>,
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
        kawaii_physics_usmap.clone(),
        kawaii_porter,
        patch_default_hidden_materials,
        default_hidden_material_bitmaps,
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
    let base_progress = installed_mods_ptr.load(Ordering::SeqCst);
    let mut pak_name = ensure_mod_name_suffix(&pak.mod_name);
    pak_name.push_str(".pak");
    let output_file = File::create(mod_dir.join(pak_name))?;

    let mut paths = vec![];
    collect_files(&mut paths, &to_pak_dir)?;

    if pak.fix_mesh {
        mesh_patch(&mut paths, &to_pak_dir.to_path_buf())?;
    }

    paths.sort();

    // Stripped audio paks match community "stripped" mods: no full directory index and
    // entries stored uncompressed; the game finds files through the path hash index.
    let mut strip = pak.strip_audio && pak.mod_type == "Audio";
    if strip {
        let rels = paths
            .iter()
            .filter_map(|p| p.strip_prefix(&to_pak_dir).ok().and_then(|r| r.to_slash()))
            .map(|r| r.into_owned())
            .collect::<Vec<_>>();
        let blockers = strip_blockers(&pak.mount_point, rels.iter().map(String::as_str));
        if blockers.is_empty() {
            // Entries Repak drops from every pak anyway never end up in a stripped one.
            paths.retain(|p| {
                !p.strip_prefix(&to_pak_dir)
                    .ok()
                    .and_then(|r| r.to_slash())
                    .is_some_and(|rel| {
                        crate::install_mod::is_unsupported_companion_entry(&pak.mount_point, &rel)
                    })
            });
        } else {
            log::warn!(
                "Not stripping {}: it has files other than audio banks ({})",
                pak.mod_name,
                summarize_strip_blockers(&blockers)
            );
            strip = false;
        }
    }
    let builder = repak::PakBuilder::new()
        .compression(if strip { vec![] } else { vec![pak.compression] })
        .full_directory_index(!strip)
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
                .build_entry(!strip, std::fs::read(p).expect("WTF"), rel)
                .expect("Failed to build entry");
            (rel.to_string(), entry)
        })
        .collect::<Vec<_>>();

    for (path, entry) in partial_entry {
        debug!("Writing: {}", path);
        pak_writer.write_entry(path, entry)?;
    }
    pak_writer.write_index()?;

    log::info!("Wrote pak file successfully (stripped: {strip})");
    let minimum_progress =
        base_progress.saturating_add(pak.total_files.max(1).min(i32::MAX as usize) as i32);
    installed_mods_ptr.fetch_max(minimum_progress, Ordering::SeqCst);
    Ok::<(), repak::Error>(())
}

#[cfg(test)]
mod tests {
    use super::{pak_contains_unsupported_entries, rewrite_unsupported_companion_pak};
    use crate::install_mod::AES_KEY;
    use repak::Version;
    use std::fs::File;

    #[test]
    fn rewrites_only_the_pak_index_without_unsupported_entries() {
        let temp = tempfile::tempdir().unwrap();
        let pak_path = temp.path().join("mod.pak");
        let mut writer = repak::PakBuilder::new().key(AES_KEY.clone().0).writer(
            File::create(&pak_path).unwrap(),
            Version::V11,
            "../../../".to_string(),
            Some(0),
        );
        writer
            .write_file("chunknames", false, b"asset list")
            .unwrap();
        writer
            .write_file("patched_files", false, b"patched asset list")
            .unwrap();
        let audio_path = "Marvel/Content/WwiseAudio/Windows/voice.wem";
        let audio_data = b"valid audio data";
        writer.write_file(audio_path, false, audio_data).unwrap();
        writer.write_index().unwrap();

        assert!(pak_contains_unsupported_entries(&pak_path).unwrap());
        assert!(rewrite_unsupported_companion_pak(&pak_path).unwrap());
        assert!(!pak_contains_unsupported_entries(&pak_path).unwrap());
        assert!(!rewrite_unsupported_companion_pak(&pak_path).unwrap());

        let reader = repak::PakBuilder::new()
            .key(AES_KEY.clone().0)
            .reader(&mut std::io::BufReader::new(File::open(&pak_path).unwrap()))
            .unwrap();
        assert_eq!(reader.files(), vec![audio_path.to_string()]);
        assert_eq!(
            reader
                .get(
                    audio_path,
                    &mut std::io::BufReader::new(File::open(&pak_path).unwrap()),
                )
                .unwrap(),
            audio_data
        );
    }

    #[test]
    fn only_audio_banks_and_media_can_be_stripped() {
        use super::{is_strippable_audio_entry as allowed, strip_blockers, summarize_strip_blockers};
        let mount = "../../../";
        assert!(allowed(mount, "Marvel/Content/WwiseAudio/bnk_sfx_1031001.bnk"));
        assert!(allowed(mount, "Marvel/Content/WwiseAudio/Media/61/123.WEM"));
        assert!(allowed("../../../Marvel/Content/WwiseAudio/", "English(US)/bnk_vo_1.bnk"));
        assert!(allowed("../../../Marvel", "Content/WwiseAudio/a.bnk"));

        assert!(!allowed(mount, "Marvel/Content/WwiseAudio/Engine.ini"));
        assert!(!allowed(mount, "Marvel/Content/WwiseAudio/a.bnk.ini"));
        assert!(!allowed(mount, "Marvel/Content/WwiseAudio/a.bnk:b.ini"));
        assert!(!allowed(mount, "Marvel/Content/WwiseAudio/../Config/a.bnk"));
        assert!(!allowed(mount, "../../../Marvel/Content/WwiseAudio/a.bnk"));
        assert!(!allowed(mount, "Marvel/Config/DefaultEngine.ini"));
        assert!(!allowed(mount, "Marvel/Content/Marvel/Characters/a.bnk"));
        assert!(!allowed(mount, "WwiseAudio/a.bnk"));
        assert!(!allowed(mount, "Marvel/Content/WwiseAudio/.bnk"));
        assert!(!allowed("../../..", "../Marvel/Content/WwiseAudio/a.bnk"));
        assert!(!allowed("", ""));

        // Entries Repak drops anyway don't block stripping; everything else does.
        let blockers = strip_blockers(
            mount,
            [
                "Marvel/Content/WwiseAudio/a.bnk",
                "patched_files",
                "Marvel/Content/WwiseAudio/Media/desktop.ini",
                "Marvel/Config/Engine.ini",
                "Marvel/Content/WwiseAudio/b.txt",
            ],
        );
        assert_eq!(
            blockers,
            vec!["Marvel/Config/Engine.ini", "Marvel/Content/WwiseAudio/b.txt"]
        );
        assert_eq!(
            summarize_strip_blockers(&blockers),
            "Marvel/Config/Engine.ini, Marvel/Content/WwiseAudio/b.txt"
        );
        let many = (1..=5).map(|i| format!("f{i}.ini")).collect::<Vec<_>>();
        assert_eq!(summarize_strip_blockers(&many), "f1.ini, f2.ini, f3.ini and 2 more");
    }
}
