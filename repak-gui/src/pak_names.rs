//! Recovers entry names for paks written without a full directory index ("stripped" paks).
//!
//! The game only needs the path hash index to load files, so some mods ship without names.
//! The names can be recovered by hashing the paths the base game ships with and matching
//! them against the mod's path hash index.

use crate::install_mod::AES_KEY;
use repak::PakReader;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, info, warn};

static GAME_PAKS_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);
static CANDIDATES: Mutex<Option<(PathBuf, Arc<Vec<String>>)>> = Mutex::new(None);

/// Content that ships as loose files inside plain paks (the only kind of mod that can be
/// stripped without breaking the game's IoStore lookups).
const CANDIDATE_PREFIXES: [&str; 2] = ["marvel/content/wwiseaudio/", "marvel/content/movies/"];

/// Sets the game Paks directory and warms the path cache in the background, so the first
/// stripped pak opened from the UI thread doesn't have to read the game paks.
pub fn set_game_paks_dir(dir: Option<PathBuf>) {
    if let Ok(mut current) = GAME_PAKS_DIR.write() {
        *current = dir.clone();
    }
    if dir.is_some() {
        std::thread::spawn(|| {
            game_candidates();
        });
    }
}

/// Opens a pak with the Rivals key and names any hash-only entries it can.
pub fn open_pak(path: &Path) -> Result<PakReader, repak::Error> {
    let mut reader = repak::PakBuilder::new()
        .key(AES_KEY.clone().0)
        .reader(&mut BufReader::new(File::open(path)?))?;
    resolve_names(&mut reader);
    Ok(reader)
}

/// Names hash-only entries using the paths found in the game's own paks.
/// Returns how many entries are still unnamed afterwards.
pub fn resolve_names(reader: &mut PakReader) -> usize {
    let unresolved = reader.unresolved_path_hashes().len();
    if unresolved == 0 {
        return 0;
    }
    let Some(candidates) = game_candidates() else {
        warn!(unresolved, "Pak has unnamed entries but the game Paks directory is unknown");
        return unresolved;
    };
    let resolved = reader.resolve_paths(candidates.iter().map(String::as_str));
    let remaining = unresolved - resolved;
    info!(resolved, remaining, "Recovered entry names from path hash index");
    remaining
}

fn game_candidates() -> Option<Arc<Vec<String>>> {
    let dir = GAME_PAKS_DIR.read().ok()?.clone()?;
    let mut cache = CANDIDATES.lock().ok()?;
    if let Some((cached_dir, candidates)) = cache.as_ref() {
        if *cached_dir == dir {
            return Some(candidates.clone());
        }
    }
    let signature = game_paks_signature(&dir);
    let candidates = match load_disk_cache(&dir, &signature) {
        Some(paths) => paths,
        None => {
            let paths = collect_candidates(&dir);
            save_disk_cache(&dir, &signature, &paths);
            paths
        }
    };
    let candidates = Arc::new(candidates);
    *cache = Some((dir, candidates.clone()));
    Some(candidates)
}

/// Reading the game paks' directory indices is the slow part (the patch pak alone has
/// over a million entries), so the collected paths are kept on disk next to the config and
/// reused until the game paks change.
#[derive(serde::Serialize, serde::Deserialize)]
struct DiskCache {
    paks_dir: PathBuf,
    signature: Vec<PakSignature>,
    paths: Vec<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct PakSignature {
    name: String,
    size: u64,
    modified_secs: u64,
}

fn disk_cache_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("repak_manager").join("game_path_cache.json"))
}

fn game_paks_signature(dir: &Path) -> Vec<PakSignature> {
    let mut signature = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("pak"))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let modified_secs = metadata
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            Some(PakSignature {
                name: entry.file_name().to_string_lossy().into_owned(),
                size: metadata.len(),
                modified_secs,
            })
        })
        .collect::<Vec<_>>();
    signature.sort_by(|a, b| a.name.cmp(&b.name));
    signature
}

fn load_disk_cache(dir: &Path, signature: &[PakSignature]) -> Option<Vec<String>> {
    let data = std::fs::read(disk_cache_path()?).ok()?;
    let cache: DiskCache = serde_json::from_slice(&data).ok()?;
    (cache.paks_dir == dir && cache.signature == signature).then(|| {
        debug!(count = cache.paths.len(), "Loaded game paths from cache");
        cache.paths
    })
}

fn save_disk_cache(dir: &Path, signature: &[PakSignature], paths: &[String]) {
    let Some(path) = disk_cache_path() else {
        return;
    };
    let cache = DiskCache {
        paks_dir: dir.to_path_buf(),
        signature: signature.to_vec(),
        paths: paths.to_vec(),
    };
    let result = serde_json::to_vec(&cache)
        .map_err(std::io::Error::other)
        .and_then(|data| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, data)
        });
    if let Err(e) = result {
        warn!(error = %e, "Failed to save game path cache");
    }
}

fn collect_candidates(dir: &Path) -> Vec<String> {
    let mut candidates = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return candidates;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("pak") {
            continue;
        }
        let reader = File::open(&path).map_err(repak::Error::Io).and_then(|file| {
            repak::PakBuilder::new()
                .key(AES_KEY.clone().0)
                .reader(&mut BufReader::new(file))
        });
        let reader = match reader {
            Ok(reader) => reader,
            Err(e) => {
                debug!(?path, error = %e, "Skipping game pak while collecting names");
                continue;
            }
        };
        let mut mount = reader
            .mount_point()
            .replace('\\', "/")
            .trim_start_matches("../../../")
            .to_string();
        if !mount.is_empty() && !mount.ends_with('/') {
            mount.push('/');
        }
        candidates.extend(reader.files().into_iter().filter_map(|file| {
            let full = format!("{mount}{file}");
            let lower = full.to_ascii_lowercase();
            CANDIDATE_PREFIXES
                .iter()
                .any(|prefix| lower.starts_with(prefix))
                .then_some(full)
        }));
    }
    info!(count = candidates.len(), ?dir, "Collected game paths for name recovery");
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_mod::install_mod_logic::pak_files::repak_dir;
    use crate::install_mod::InstallableMod;
    use std::sync::atomic::AtomicI32;

    /// Decodes a real stripped audio mod using the installed game's paks, then strips it
    /// again and checks the output matches the original byte for byte.
    /// Run with STRIPPED_PAK=<mod .pak> RIVALS_PAKS=<game Paks dir> cargo test -- --ignored
    #[test]
    #[ignore]
    fn decodes_and_restrips_real_audio_mod() {
        let (Ok(pak), Ok(paks)) = (std::env::var("STRIPPED_PAK"), std::env::var("RIVALS_PAKS"))
        else {
            return;
        };
        let pak = PathBuf::from(pak);
        set_game_paks_dir(Some(PathBuf::from(paks)));

        let reader = open_pak(&pak).unwrap();
        assert!(!reader.has_full_directory_index());
        assert!(reader.unresolved_path_hashes().is_empty());
        let files = reader.files();
        println!("recovered: {files:?}");

        let raw = tempfile::tempdir().unwrap();
        for file in &files {
            let data = reader
                .get(file, &mut BufReader::new(File::open(&pak).unwrap()))
                .unwrap();
            let out = raw.path().join(file);
            std::fs::create_dir_all(out.parent().unwrap()).unwrap();
            std::fs::write(out, data).unwrap();
        }

        let out = tempfile::tempdir().unwrap();
        let installable = InstallableMod {
            mod_name: "restripped_9999999_P".into(),
            mod_type: "Audio".into(),
            strip_audio: true,
            mount_point: reader.mount_point().into(),
            path_hash_seed: reader.path_hash_seed().unwrap_or_default().to_string(),
            total_files: files.len(),
            ..Default::default()
        };
        repak_dir(&installable, raw.path().into(), out.path().into(), &AtomicI32::new(0)).unwrap();

        let original = std::fs::read(&pak).unwrap();
        let rewritten = std::fs::read(out.path().join("restripped_9999999_P.pak")).unwrap();
        assert_eq!(original.len(), rewritten.len(), "size differs");
        let first_diff = original.iter().zip(&rewritten).position(|(a, b)| a != b);
        assert_eq!(first_diff, None, "first differing byte");
    }

    /// Strips a real unstripped audio mod in place and checks every bank survives.
    /// Run with UNSTRIPPED_PAK=<mod .pak> RIVALS_PAKS=<game Paks dir> cargo test -- --ignored
    #[test]
    #[ignore]
    fn strips_installed_audio_mod_in_place() {
        let (Ok(source), Ok(paks)) =
            (std::env::var("UNSTRIPPED_PAK"), std::env::var("RIVALS_PAKS"))
        else {
            return;
        };
        set_game_paks_dir(Some(PathBuf::from(paks)));
        let dir = tempfile::tempdir().unwrap();
        let pak = dir.path().join("audio_9999999_P.pak");
        std::fs::copy(source, &pak).unwrap();

        let before = open_pak(&pak).unwrap();
        let mut source = BufReader::new(File::open(&pak).unwrap());
        let expected = before
            .files()
            .into_iter()
            .filter(|f| !crate::install_mod::is_unsupported_companion_entry(before.mount_point(), f))
            .map(|f| (before.get(&f, &mut source).unwrap(), f))
            .collect::<Vec<_>>();
        drop(source);

        crate::install_mod::install_mod_logic::pak_files::strip_installed_pak(&pak, true).unwrap();
        assert!(dir.path().join("audio_9999999_P.pak.rebak").exists());

        let after = open_pak(&pak).unwrap();
        assert!(!after.has_full_directory_index());
        assert!(after.unresolved_path_hashes().is_empty(), "names not recovered");
        let mut source = BufReader::new(File::open(&pak).unwrap());
        for (data, file) in &expected {
            assert_eq!(&after.get(file, &mut source).unwrap(), data, "{file}");
        }
        println!("stripped {} entries: {:?}", expected.len(), after.files());
    }
}
