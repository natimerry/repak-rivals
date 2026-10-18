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

pub fn set_game_paks_dir(dir: Option<PathBuf>) {
    if let Ok(mut current) = GAME_PAKS_DIR.write() {
        *current = dir;
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
    let candidates = Arc::new(collect_candidates(&dir));
    *cache = Some((dir, candidates.clone()));
    Some(candidates)
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
}
