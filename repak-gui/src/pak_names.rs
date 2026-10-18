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

