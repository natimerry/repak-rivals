use semver::Version;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, instrument};
use walkdir::WalkDir;
use zip::ZipArchive;

const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/natimerry/repak-rivals/releases/latest";
const RAW_CHANGELOG_BASE: &str = "https://raw.githubusercontent.com/natimerry/repak-rivals";
const USER_AGENT: &str = "repak-rivals-native-updater";

#[derive(Clone, Debug)]
pub struct AvailableUpdate {
    pub current_version: Version,
    pub latest_version: Version,
    pub tag_name: String,
    pub asset_name: String,
    pub asset_download_url: String,
    pub changelog: String,
}

#[derive(Clone, Debug)]
pub struct PreparedUpdate {
    pub source_dir: PathBuf,
    pub install_dir: PathBuf,
    pub restart_exe: PathBuf,
}

#[derive(Debug)]
pub enum UpdateMessage {
    CheckFinished(Result<Option<AvailableUpdate>, String>),
    InstallProgress(String),
    InstallFinished(Result<PreparedUpdate, String>),
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[instrument(skip(current_version))]
pub fn check_for_update(current_version: &str) -> Result<Option<AvailableUpdate>, String> {
    let client = reqwest::blocking::Client::new();
    let release_body = client
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Failed to query GitHub releases: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub release query failed: {e}"))?
        .text()
        .map_err(|e| format!("Failed to read GitHub release data: {e}"))?;
    let release: GithubRelease = serde_json::from_str(&release_body)
        .map_err(|e| format!("Failed to parse GitHub release data: {e}"))?;

    let latest = release.tag_name.trim_start_matches('v');
    let latest_version =
        Version::parse(latest).map_err(|e| format!("Invalid latest version `{latest}`: {e}"))?;
    let current_version = Version::parse(current_version)
        .map_err(|e| format!("Invalid current version `{current_version}`: {e}"))?;

    if current_version >= latest_version {
        return Ok(None);
    }

    let asset = choose_release_asset(&release.assets, false)
        .ok_or_else(|| "No matching repak-gui .zip release asset was found".to_string())?;
    let changelog = fetch_release_changelog(
        &client,
        &release.tag_name,
        &current_version,
        &latest_version,
        release.body.as_deref(),
    );

    Ok(Some(AvailableUpdate {
        current_version,
        latest_version,
        tag_name: release.tag_name,
        asset_name: asset.name.clone(),
        asset_download_url: asset.browser_download_url.clone(),
        changelog,
    }))
}

pub fn check_for_self_contained_repak_gui(
    current_version: &str,
) -> Result<AvailableUpdate, String> {
    let client = reqwest::blocking::Client::new();
    let release_body = client
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Failed to query GitHub releases: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub release query failed: {e}"))?
        .text()
        .map_err(|e| format!("Failed to read GitHub release data: {e}"))?;
    let release: GithubRelease = serde_json::from_str(&release_body)
        .map_err(|e| format!("Failed to parse GitHub release data: {e}"))?;

    let latest = release.tag_name.trim_start_matches('v');
    let latest_version =
        Version::parse(latest).map_err(|e| format!("Invalid latest version `{latest}`: {e}"))?;
    let current_version = Version::parse(current_version)
        .map_err(|e| format!("Invalid current version `{current_version}`: {e}"))?;

    let asset = choose_release_asset(&release.assets, true).ok_or_else(|| {
        "No matching self-contained repak-gui release asset was found for this platform".to_string()
    })?;
    let mut changelog = fetch_release_changelog(
        &client,
        &release.tag_name,
        &current_version,
        &latest_version,
        release.body.as_deref(),
    );
    changelog = format!(
        "## Self-contained repak-gui\n\nThis will replace the current repak-gui with the self-contained build for your platform. The bundled KawaiiPhysics helper does not need a locally installed .NET runtime.\n\n{changelog}"
    );

    Ok(AvailableUpdate {
        current_version,
        latest_version,
        tag_name: release.tag_name,
        asset_name: asset.name.clone(),
        asset_download_url: asset.browser_download_url.clone(),
        changelog,
    })
}

pub fn download_and_prepare_update(
    update: AvailableUpdate,
    progress: Sender<UpdateMessage>,
) -> Result<PreparedUpdate, String> {
    let current_exe =
        std::env::current_exe().map_err(|e| format!("Failed to locate current executable: {e}"))?;
    let install_dir = current_exe
        .parent()
        .ok_or_else(|| {
            format!(
                "Executable has no parent directory: {}",
                current_exe.display()
            )
        })?
        .to_path_buf();

    let stage_dir = update_stage_dir(&update.tag_name)?;
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir)
            .map_err(|e| format!("Failed to clean {}: {e}", stage_dir.display()))?;
    }
    fs::create_dir_all(&stage_dir)
        .map_err(|e| format!("Failed to create {}: {e}", stage_dir.display()))?;

    send_progress(
        &progress,
        format!("Downloading {}", update.asset_name.as_str()),
    );
    let bytes = reqwest::blocking::Client::new()
        .get(&update.asset_download_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Failed to download {}: {e}", update.asset_name))?
        .error_for_status()
        .map_err(|e| format!("Release asset download failed: {e}"))?
        .bytes()
        .map_err(|e| format!("Failed to read release asset: {e}"))?;

    send_progress(&progress, format!("Downloaded {} bytes", bytes.len()));
    let extract_dir = stage_dir.join("extracted");
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create {}: {e}", extract_dir.display()))?;
    extract_release_asset(&update.asset_name, &bytes, &extract_dir)?;

    let staged_exe = find_staged_executable(&extract_dir, &current_exe)?;
    let source_dir = release_root_for_exe(&extract_dir, &staged_exe);
    send_progress(
        &progress,
        format!("Prepared update from {}", source_dir.display()),
    );

    Ok(PreparedUpdate {
        source_dir,
        install_dir,
        restart_exe: current_exe,
    })
}

pub fn spawn_replace_and_restart(prepared: &PreparedUpdate) -> Result<(), String> {
    info!(
        source_dir = %prepared.source_dir.display(),
        install_dir = %prepared.install_dir.display(),
        restart_exe = %prepared.restart_exe.display(),
        "Spawning updater helper"
    );

    #[cfg(target_os = "windows")]
    {
        spawn_windows_replace_and_restart(prepared)
    }

    #[cfg(not(target_os = "windows"))]
    {
        spawn_unix_replace_and_restart(prepared)
    }
}

fn choose_release_asset(assets: &[GithubAsset], self_contained: bool) -> Option<&GithubAsset> {
    assets
        .iter()
        .filter(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.starts_with("repak-gui")
                && platform_matches(&name)
                && supported_archive(&name)
                && name.contains("self-contained") == self_contained
        })
        .min_by_key(|asset| {
            let name = asset.name.to_ascii_lowercase();
            (!name.contains(std::env::consts::ARCH), name.len())
        })
}

fn supported_archive(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        name.ends_with(".zip")
    } else {
        name.ends_with(".tar.xz") || name.ends_with(".zip")
    }
}

fn platform_matches(name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        name.contains("windows") || name.contains("pc-windows") || name.contains("msvc")
    }

    #[cfg(target_os = "linux")]
    {
        name.contains("linux") || name.contains("unknown-linux")
    }

    #[cfg(target_os = "macos")]
    {
        name.contains("apple") || name.contains("darwin") || name.contains("macos")
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        true
    }
}

fn fetch_release_changelog(
    client: &reqwest::blocking::Client,
    tag_name: &str,
    current_version: &Version,
    latest_version: &Version,
    release_body: Option<&str>,
) -> String {
    let url = format!("{RAW_CHANGELOG_BASE}/{tag_name}/CHANGELOG.md");
    match client
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text())
    {
        Ok(markdown) => {
            let parsed = parse_changelog_range(&markdown, current_version, latest_version);
            if !parsed.trim().is_empty() {
                return parsed;
            }
        }
        Err(err) => {
            debug!(error = %err, "Failed to fetch tagged CHANGELOG.md");
        }
    }

    release_body
        .filter(|body| !body.trim().is_empty())
        .unwrap_or("No changelog details were published for this release.")
        .to_string()
}

fn parse_changelog_range(
    markdown: &str,
    current_version: &Version,
    latest_version: &Version,
) -> String {
    let mut sections = Vec::new();
    let mut current_heading: Option<(Version, String)> = None;
    let mut current_body = Vec::new();

    for line in markdown.lines() {
        if let Some((version, heading)) = parse_version_heading(line) {
            flush_changelog_section(
                &mut sections,
                current_heading.take(),
                &mut current_body,
                current_version,
                latest_version,
            );
            current_heading = Some((version, heading));
        } else if current_heading.is_some() {
            current_body.push(line.to_string());
        }
    }

    flush_changelog_section(
        &mut sections,
        current_heading,
        &mut current_body,
        current_version,
        latest_version,
    );

    sections.join("\n\n")
}

fn parse_version_heading(line: &str) -> Option<(Version, String)> {
    let trimmed = line.trim();
    let heading = trimmed.strip_prefix("## ")?;
    let version_text = heading
        .strip_prefix("Version ")
        .unwrap_or(heading)
        .split_whitespace()
        .next()?
        .trim_start_matches('v')
        .trim_matches(|c| c == '(' || c == ')');
    let version = Version::parse(version_text).ok()?;
    Some((version, heading.to_string()))
}

fn flush_changelog_section(
    sections: &mut Vec<String>,
    heading: Option<(Version, String)>,
    body: &mut Vec<String>,
    current_version: &Version,
    latest_version: &Version,
) {
    let Some((version, heading)) = heading else {
        body.clear();
        return;
    };

    if version > *current_version && version <= *latest_version {
        let body_text = body.join("\n").trim().to_string();
        sections.push(if body_text.is_empty() {
            format!("## {heading}")
        } else {
            format!("## {heading}\n{body_text}")
        });
    }

    body.clear();
}

fn update_stage_dir(tag_name: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System time is before UNIX_EPOCH: {e}"))?
        .as_secs();
    let safe_tag = tag_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    Ok(dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("repak_manager")
        .join("updates")
        .join(format!("{safe_tag}-{timestamp}")))
}

fn extract_zip(bytes: &[u8], output_dir: &Path) -> Result<(), String> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Invalid zip archive: {e}"))?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("Failed to read zip entry {index}: {e}"))?;
        let Some(enclosed_name) = file.enclosed_name() else {
            continue;
        };
        let output_path = output_dir.join(enclosed_name);

        if file.is_dir() {
            fs::create_dir_all(&output_path)
                .map_err(|e| format!("Failed to create {}: {e}", output_path.display()))?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }

        let mut output = fs::File::create(&output_path)
            .map_err(|e| format!("Failed to create {}: {e}", output_path.display()))?;
        std::io::copy(&mut file, &mut output)
            .map_err(|e| format!("Failed to extract {}: {e}", output_path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&output_path, fs::Permissions::from_mode(mode)).map_err(
                    |e| {
                        format!(
                            "Failed to set permissions on {}: {e}",
                            output_path.display()
                        )
                    },
                )?;
            }
        }
    }

    Ok(())
}

fn extract_release_asset(asset_name: &str, bytes: &[u8], output_dir: &Path) -> Result<(), String> {
    let lower = asset_name.to_ascii_lowercase();
    if lower.ends_with(".zip") {
        extract_zip(bytes, output_dir)
    } else if lower.ends_with(".tar.xz") {
        extract_tar_xz(bytes, output_dir)
    } else {
        Err(format!("Unsupported release archive format: {asset_name}"))
    }
}

fn extract_tar_xz(bytes: &[u8], output_dir: &Path) -> Result<(), String> {
    let archive_path = output_dir.join("repak-gui-update.tar.xz");
    fs::write(&archive_path, bytes)
        .map_err(|e| format!("Failed to stage {}: {e}", archive_path.display()))?;

    let status = Command::new("tar")
        .arg("-xJf")
        .arg(&archive_path)
        .arg("-C")
        .arg(output_dir)
        .status()
        .map_err(|e| format!("Failed to start tar: {e}"))?;
    if !status.success() {
        return Err(format!(
            "tar failed while extracting {asset_name}",
            asset_name = archive_path.display()
        ));
    }
    let _ = fs::remove_file(archive_path);
    Ok(())
}

fn find_staged_executable(extract_dir: &Path, current_exe: &Path) -> Result<PathBuf, String> {
    let current_name = current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "Current executable has no file name: {}",
                current_exe.display()
            )
        })?;

    let mut fallback = None;
    for entry in WalkDir::new(extract_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if file_name == current_name {
            return Ok(path.to_path_buf());
        }

        let lower = file_name.to_ascii_lowercase();
        if fallback.is_none()
            && lower.contains("repak-gui")
            && (cfg!(not(target_os = "windows")) || lower.ends_with(".exe"))
        {
            fallback = Some(path.to_path_buf());
        }
    }

    fallback.ok_or_else(|| {
        format!(
            "Downloaded release did not contain a replacement for {}",
            current_name
        )
    })
}

fn release_root_for_exe(extract_dir: &Path, staged_exe: &Path) -> PathBuf {
    let Ok(children) = fs::read_dir(extract_dir) else {
        return extract_dir.to_path_buf();
    };
    let children = children.filter_map(Result::ok).collect::<Vec<_>>();
    if children.len() == 1 {
        let only_child = children[0].path();
        if only_child.is_dir() && staged_exe.starts_with(&only_child) {
            return only_child;
        }
    }

    staged_exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| extract_dir.to_path_buf())
}

fn send_progress(progress: &Sender<UpdateMessage>, message: String) {
    let _ = progress.send(UpdateMessage::InstallProgress(message));
}

#[cfg(target_os = "windows")]
fn spawn_windows_replace_and_restart(prepared: &PreparedUpdate) -> Result<(), String> {
    let script = prepared.source_dir.join("repak-rivals-apply-update.ps1");
    fs::write(
        &script,
        r#"
param(
    [Parameter(Mandatory=$true)][string]$SourceDir,
    [Parameter(Mandatory=$true)][string]$InstallDir,
    [Parameter(Mandatory=$true)][string]$RestartExe,
    [Parameter(Mandatory=$true)][int]$PidToWait
)

while (Get-Process -Id $PidToWait -ErrorAction SilentlyContinue) {
    Start-Sleep -Milliseconds 300
}

Get-ChildItem -Force -LiteralPath $SourceDir | Copy-Item -Destination $InstallDir -Recurse -Force
Start-Process -FilePath $RestartExe -WorkingDirectory $InstallDir
Remove-Item -LiteralPath $PSCommandPath -Force
"#,
    )
    .map_err(|e| format!("Failed to write updater helper {}: {e}", script.display()))?;

    Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg(&prepared.source_dir)
        .arg(&prepared.install_dir)
        .arg(&prepared.restart_exe)
        .arg(std::process::id().to_string())
        .spawn()
        .map_err(|e| format!("Failed to start updater helper: {e}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn spawn_unix_replace_and_restart(prepared: &PreparedUpdate) -> Result<(), String> {
    let script = prepared.source_dir.join("repak-rivals-apply-update.sh");
    let script_body = r#"#!/bin/sh
set -eu
source_dir="$1"
install_dir="$2"
restart_exe="$3"
pid_to_wait="$4"

while kill -0 "$pid_to_wait" 2>/dev/null; do
    sleep 0.3
done

cp -R "$source_dir"/. "$install_dir"/
chmod +x "$restart_exe" 2>/dev/null || true
cd "$install_dir"
"$restart_exe" >/dev/null 2>&1 &
rm -f "$0"
"#;
    fs::write(&script, script_body)
        .map_err(|e| format!("Failed to write updater helper {}: {e}", script.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to mark updater helper executable: {e}"))?;
    }

    Command::new(&script)
        .arg(&prepared.source_dir)
        .arg(&prepared.install_dir)
        .arg(&prepared.restart_exe)
        .arg(std::process::id().to_string())
        .spawn()
        .map_err(|e| format!("Failed to start updater helper: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_changelog_sections_newer_than_current() {
        let changelog = r#"# Changelog

## Version 3.2.4

- Newest fix.

## Version 3.2.3

- Previous fix.

## Version 3.2.2

- Old fix.
"#;

        let parsed = parse_changelog_range(
            changelog,
            &Version::parse("3.2.2").unwrap(),
            &Version::parse("3.2.4").unwrap(),
        );

        assert!(parsed.contains("Version 3.2.4"));
        assert!(parsed.contains("Version 3.2.3"));
        assert!(!parsed.contains("Version 3.2.2"));
    }
}
