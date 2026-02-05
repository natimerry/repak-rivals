use std::collections::HashMap;
use std::option::Option;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{fs, io};

#[derive(Debug, Deserialize, Serialize, Hash)]
struct SkinEntry {
    skinid: String,
    #[serde(rename = "skin_name")]
    skin_name: String,
    name: String,
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
                None => return "Character (Unknown)".to_string(),
            },
            "UI" => return "UI".to_string(),
            "Movies" => return "Movies".to_string(),
            _ if path.contains("WwiseAudio") => return "Audio".to_string(),
            _ => {}
        }
    }

    fallback.unwrap_or_else(|| "Unknown".to_string())
}

use log::info;
use regex_lite::Regex;
use serde::{Deserialize, Serialize};

pub fn find_marvel_rivals() -> Option<PathBuf> {
    let shit = get_steam_library_paths();
    if shit.is_empty() {
        return None;
    }

    for lib in shit {
        let path = lib.join("steamapps/common/MarvelRivals/MarvelGame/Marvel/Content/Paks");
        if path.exists() {
            return Some(path);
        }
    }
    println!("Marvel Rivals not found.");
    None
}

/// Reads `libraryfolders.vdf` to find additional Steam libraries.
fn get_steam_library_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    let vdf_path = PathBuf::from("C:/Program Files (x86)/Steam/steamapps/libraryfolders.vdf");

    #[cfg(target_os = "linux")]
    let vdf_path = PathBuf::from("~/.steam/steam/steamapps/libraryfolders.vdf");

    if !vdf_path.exists() {
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
            info!("Found steam library path: {:?}", path);
            paths.push(path.unwrap());
        }
    }

    paths
}
