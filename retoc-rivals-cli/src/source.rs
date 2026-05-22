use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IoStorePackage {
    pub pak: PathBuf,
    pub utoc: PathBuf,
    pub ucas: PathBuf,
}

impl IoStorePackage {
    pub fn stem(&self) -> String {
        self.pak
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("mod")
            .to_string()
    }
}

#[derive(Clone, Debug)]
pub enum PackageSource {
    IoStore(IoStorePackage),
    LegacyPak(PathBuf),
    RawDirectory(PathBuf),
    Archive(PathBuf),
    DirectoryPackages {
        root: PathBuf,
        iostore: Vec<IoStorePackage>,
        legacy_paks: Vec<PathBuf>,
    },
}

pub fn classify_path(path: &Path) -> Result<PackageSource, String> {
    if path.is_dir() {
        let (iostore, legacy_paks) = scan_directory_packages(path);
        if !has_uasset(path) && (!iostore.is_empty() || !legacy_paks.is_empty()) {
            return Ok(PackageSource::DirectoryPackages {
                root: path.to_path_buf(),
                iostore,
                legacy_paks,
            });
        }
        return Ok(PackageSource::RawDirectory(path.to_path_buf()));
    }

    let ext = lower_ext(path);
    if ext.as_deref() == Some("rar") || ext.as_deref() == Some("zip") {
        return Ok(PackageSource::Archive(path.to_path_buf()));
    }

    if matches!(ext.as_deref(), Some("pak" | "utoc" | "ucas")) {
        return classify_package_file(path);
    }

    Err(format!("Unsupported input: {}", path.display()))
}

pub fn classify_package_file(path: &Path) -> Result<PackageSource, String> {
    let pak = companion_path(path, "pak").unwrap_or_else(|| path.with_extension("pak"));
    let utoc = companion_path(path, "utoc").unwrap_or_else(|| path.with_extension("utoc"));
    let ucas = companion_path(path, "ucas").unwrap_or_else(|| path.with_extension("ucas"));

    if pak.exists() && utoc.exists() && ucas.exists() {
        return Ok(PackageSource::IoStore(IoStorePackage { pak, utoc, ucas }));
    }

    if lower_ext(path).as_deref() == Some("pak") && pak.exists() {
        return Ok(PackageSource::LegacyPak(pak));
    }

    Err(format!(
        "Missing IoStore companions for {}. Need .pak, .utoc, and .ucas with same stem",
        path.display()
    ))
}

pub fn scan_directory_packages(dir: &Path) -> (Vec<IoStorePackage>, Vec<PathBuf>) {
    let mut seen_iostore = HashSet::new();
    let mut iostore = Vec::new();
    let mut legacy_paks = Vec::new();

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !matches!(lower_ext(path).as_deref(), Some("pak" | "utoc" | "ucas")) {
            continue;
        }

        match classify_package_file(path) {
            Ok(PackageSource::IoStore(pkg)) => {
                if seen_iostore.insert(pkg.utoc.clone()) {
                    iostore.push(pkg);
                }
            }
            Ok(PackageSource::LegacyPak(pak)) => {
                if lower_ext(path).as_deref() == Some("pak") {
                    legacy_paks.push(pak);
                }
            }
            _ => {}
        }
    }

    iostore.sort_by(|a, b| a.utoc.cmp(&b.utoc));
    legacy_paks.sort();
    legacy_paks.dedup();
    (iostore, legacy_paks)
}

fn has_uasset(dir: &Path) -> bool {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("uasset"))
        })
}

fn lower_ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn companion_path(path: &Path, wanted_ext: &str) -> Option<PathBuf> {
    let stem = path.file_stem()?.to_str()?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    for entry in std::fs::read_dir(parent).ok()? {
        let path = entry.ok()?.path();
        let matches_stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value == stem);
        let matches_ext = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(wanted_ext));
        if matches_stem && matches_ext {
            return Some(path);
        }
    }

    None
}
