use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

static SKIN_ENTRIES: LazyLock<HashMap<u32, SkinEntry>> = LazyLock::new(|| {
    let json_data = fs::read_to_string("data/character_data.json")
        .unwrap_or_else(|_| include_str!("../../repak-gui/src/data/character_data.json").to_owned());

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

static SKIN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9]{4}\/[0-9]{7}").unwrap());

pub enum ModType {
    Default(String),
    Custom(String),
}

pub fn collect_files(paths: &mut Vec<PathBuf>, dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(paths, &path)?;
        } else {
            paths.push(path);
        }
    }
    Ok(())
}

pub fn get_character_mod_skin(file: &str) -> Option<ModType> {
    let skin_id = SKIN_REGEX.clone().captures(file)?;
    let skin_id = skin_id[0].to_string();
    let skin_id = &skin_id[5..];
    let skin = SKIN_ENTRIES.get(&(skin_id.parse().ok()?))?;
    if skin.skin_name == "Default" {
        return Some(ModType::Default(format!("{} - {}", skin.name, skin.skin_name)));
    }

    Some(ModType::Custom(format!("{} - {}", skin.name, skin.skin_name)))
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
