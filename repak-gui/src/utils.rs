use crate::install_mod::InstallableMod;
use path_clean::PathClean;
use std::collections::HashMap;
use std::option::Option;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs, io};

#[derive(Debug, Deserialize, Serialize, Hash)]
pub struct SkinEntry {
    pub skinid: String,
    #[serde(rename = "skin_name")]
    pub skin_name: String,
    pub name: String,
}
// we grab the locally found character_data.json otherwise we let the program use the build time
// provided one
static SKIN_ENTRIES: LazyLock<HashMap<u32, SkinEntry>> = LazyLock::new(|| {
    let json_data = match fs::read_to_string("data/character_data.json") {
        Ok(s) => s,
        Err(_) => include_str!("data/character_data.json").to_owned(),
    };

    let skins: Vec<SkinEntry> =
        serde_json::from_str(&json_data).expect("Invalid character_data.json");
    skins
        .into_iter()
        .map(|entry| {
            let id: u32 = entry.skinid.parse().expect("Invalid skinid");
            (id, entry)
        })
        .collect()
});

static SKIN_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9]{4}\/[0-9]{7}").unwrap());

pub fn collect_files(paths: &mut Vec<PathBuf>, dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(paths, &path)?;
        } else {
            paths.push(entry.path());
        }
    }
    Ok(())
}

pub enum ModType {
    Default(String),
    Custom(String),
}
pub fn get_character_mod_skin(file: &str) -> Option<ModType> {
    let skin_id = SKIN_REGEX.clone().captures(file);
    if let Some(skin_id) = skin_id {
        let skin_id = skin_id[0].to_string();
        let skin_id = &skin_id[5..];
        let skin = SKIN_ENTRIES.get(&(skin_id.parse().unwrap()));
        if let Some(skin) = skin {
            if skin.skin_name == "Default" {
                return Some(ModType::Default(format!(
                    "{} - {}",
                    &skin.name, &skin.skin_name
                )));
            }
            return Some(ModType::Custom(format!(
                "{} - {}",
                &skin.name, &skin.skin_name
            )));
        }
        None
    } else {
        None
    }
}
pub fn get_current_pak_characteristics(mod_contents: Vec<String>) -> String {
    let mut fallback: Option<String> = None;

    for file in &mod_contents {
        let path = file
            .strip_prefix("Marvel/Content/Marvel/")
            .or_else(|| file.strip_prefix("/Game/Marvel/"))
            .unwrap_or(file);

        let category = path.split('/').next().unwrap_or_default();

        match category {
            "Characters" => match get_character_mod_skin(path) {
                Some(ModType::Custom(skin)) => return skin,
                Some(ModType::Default(name)) => fallback = Some(name),
                None => {}
            },
            "UI" => return "UI".to_string(),
            "Movies" => return "Movies".to_string(),
            _ if path.contains("WwiseAudio") => return "Audio".to_string(),
            _ => {}
        }
    }

    fallback.unwrap_or_else(|| "Character (Unknown)".to_string())
}

use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

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

#[instrument]
pub fn find_marvel_rivals() -> Option<PathBuf> {
    let library_paths = get_steam_library_paths();
    if library_paths.is_empty() {
        warn!("No Steam library paths were detected");
        return None;
    }

    for lib in library_paths {
        let path = lib.join("steamapps/common/MarvelRivals/MarvelGame/Marvel/Content/Paks");
        if path.exists() {
            info!(?path, "Detected Marvel Rivals install");
            return Some(path);
        }
    }
    warn!("Marvel Rivals install path was not found in discovered Steam libraries");
    None
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

pub fn latest_depot_usmap_path(current: Option<&Path>) -> Result<Option<PathBuf>, String> {
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

pub fn mods_need_kawaii_mapping(mods: &[InstallableMod]) -> bool {
    mods.iter()
        .any(|mods| mods.enabled && mods.kawaii_porter && (mods.is_dir || mods.repak))
}

pub fn match_exact_paks_suffix(path: &Path) -> Option<PathBuf> {
    let target = ["MarvelRivals", "MarvelGame", "Marvel", "Content", "Paks"];

    let components: Vec<_> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    let start = components.iter().position(|c| *c == "MarvelRivals")?;
    let remaining = &components[start..];

    if remaining.len() < target.len() || remaining[..target.len()] != target {
        return None;
    }

    if remaining.len() != target.len() {
        return None;
    }

    let mut result = PathBuf::new();
    for c in &components[..start + target.len()] {
        result.push(c);
    }

    Some(result)
}

/// Reads `libraryfolders.vdf` to find additional Steam libraries.
#[instrument]
fn get_steam_library_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    let vdf_path = PathBuf::from("C:/Program Files (x86)/Steam/steamapps/libraryfolders.vdf");

    #[cfg(target_os = "linux")]
    let vdf_path = {
        let home = dirs::home_dir().unwrap();
        let path = home.join(".steam/steam/steamapps/libraryfolders.vdf");
        path
    };
    if !vdf_path.exists() {
        debug!(?vdf_path, "Steam library manifest not found");
        return vec![];
    }

    let content = fs::read_to_string(vdf_path).ok().unwrap_or_default();
    let mut paths = Vec::new();

    for line in content.lines() {
        // if line.contains('"') {
        //     let path: String = line
        //         .split('"')
        //         .nth(3)  // Extracts the path
        //         .map(|s| s.replace("\\\\", "/"))?; // Fix Windows paths
        //     paths.push(PathBuf::from(path).join("steamapps/common"));
        // }
        if line.trim().starts_with("\"path\"") {
            let path = line
                .split("\"")
                .nth(3)
                .map(|s| PathBuf::from(s.replace("\\\\", "\\")));
            info!(?path, "Found Steam library path");
            paths.push(path.unwrap());
        }
    }

    paths
}
