use std::{
    fs,
    path::{Path, PathBuf},
};

use path_clean::PathClean;
use serde::Deserialize;
use tracing::info;

const RIVALS_USMAP_API_URL: &str =
    "https://api.github.com/repos/SpaceDepot/rivals-depot/contents/usmap?ref=main";
const RIVALS_USMAP_USER_AGENT: &str = "repak-rivals-usmap-updater";

#[derive(Debug, Deserialize)]
struct GithubContentEntry {
    name: String,
    download_url: Option<String>,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug)]
struct DepotUsmap {
    name: String,
    build: u64,
    download_url: String,
}

fn usmap_build_from_name(name: &str) -> Option<u64> {
    let file_name = Path::new(name).file_name()?.to_str()?;
    if !file_name.ends_with(".usmap") {
        return None;
    }

    let (_, suffix) = file_name.split_once('-')?;
    let digits = suffix
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }

    digits.parse().ok()
}

fn usmap_build_from_path(path: &Path) -> Option<u64> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(usmap_build_from_name)
}

fn query_latest_depot_usmap() -> Result<DepotUsmap, String> {
    let client = reqwest::blocking::Client::new();
    let entries = client
        .get(RIVALS_USMAP_API_URL)
        .header("User-Agent", RIVALS_USMAP_USER_AGENT)
        .send()
        .map_err(|e| format!("Failed to query rivals-depot usmaps: {e}"))?
        .error_for_status()
        .map_err(|e| format!("rivals-depot usmap query failed: {e}"))?
        .text()
        .map_err(|e| format!("Failed to read rivals-depot usmap response: {e}"))?;

    let entries = serde_json::from_str::<Vec<GithubContentEntry>>(&entries)
        .map_err(|e| format!("Failed to parse rivals-depot usmap response: {e}"))?;

    entries
        .into_iter()
        .filter(|entry| entry.kind == "file")
        .filter_map(|entry| {
            let build = usmap_build_from_name(&entry.name)?;
            let download_url = entry.download_url?;
            Some(DepotUsmap {
                name: entry.name,
                build,
                download_url,
            })
        })
        .max_by_key(|entry| entry.build)
        .ok_or_else(|| "rivals-depot did not return any usable .usmap files".to_string())
}

fn download_depot_usmap(usmap: &DepotUsmap) -> Result<PathBuf, String> {
    let usmap_dir = PathBuf::from("usmap");
    fs::create_dir_all(&usmap_dir)
        .map_err(|e| format!("Failed to create {}: {e}", usmap_dir.display()))?;

    let output_path = usmap_dir.join(&usmap.name);
    if output_path.exists() {
        return Ok(output_path.clean());
    }

    let client = reqwest::blocking::Client::new();
    let bytes = client
        .get(&usmap.download_url)
        .header("User-Agent", RIVALS_USMAP_USER_AGENT)
        .send()
        .map_err(|e| format!("Failed to download {}: {e}", usmap.name))?
        .error_for_status()
        .map_err(|e| format!("Download failed for {}: {e}", usmap.name))?
        .bytes()
        .map_err(|e| format!("Failed to read {} download: {e}", usmap.name))?;

    fs::write(&output_path, bytes)
        .map_err(|e| format!("Failed to write {}: {e}", output_path.display()))?;
    Ok(output_path.clean())
}

fn latest_depot_usmap_path(current: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let latest = query_latest_depot_usmap()?;
    let current_build = current.and_then(usmap_build_from_path);
    if current.is_some_and(Path::exists) && current_build.is_some_and(|build| build >= latest.build)
    {
        return Ok(None);
    }

    let path = download_depot_usmap(&latest)?;
    info!(
        usmap = %path.display(),
        build = latest.build,
        previous_build = ?current_build,
        "Using latest rivals-depot mapping"
    );
    Ok(Some(path))
}

pub fn resolve_kawaii_usmap(current: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = latest_depot_usmap_path(current)? {
        return Ok(path);
    }

    current
        .map(Path::to_path_buf)
        .ok_or_else(|| "No KawaiiPhysics USMAP was provided or downloadable".to_string())
}
