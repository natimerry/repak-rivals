use crate::utils::find_marvel_rivals;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};

const STEAM_APP_URL: &str = "steam://run/2767030";
const GAME_EXE_NAME: &str = "Marvel-Win64-Shipping.exe";

#[derive(Clone, Debug)]
pub(crate) struct GameLaunchPaths {
    pub(crate) paks_path: PathBuf,
    launch_record: PathBuf,
}

#[derive(Clone, Debug)]
enum LaunchRecordBackup {
    Present(String),
    Missing,
}

pub(crate) fn detect_game_launch_paths() -> Result<GameLaunchPaths, String> {
    let paks_path = find_marvel_rivals()
        .ok_or_else(|| "Could not detect Marvel Rivals in Steam libraries".to_string())?;
    derive_game_launch_paths_from_paks(&paks_path)
}

fn derive_game_launch_paths_from_paks(paks_path: &Path) -> Result<GameLaunchPaths, String> {
    if !paks_path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Paks"))
    {
        return Err(format!(
            "Detected path is not a Paks folder: {}",
            paks_path.display()
        ));
    }

    let content_dir = parent_named(paks_path, "Content")?;
    let marvel_dir = parent_named(content_dir, "Marvel")?;
    let game_root = parent_named(marvel_dir, "MarvelGame")?
        .parent()
        .ok_or_else(|| "Could not determine Marvel Rivals game root".to_string())?
        .to_path_buf();

    Ok(GameLaunchPaths {
        paks_path: paks_path.to_path_buf(),
        launch_record: game_root.join("launch_record"),
    })
}

pub(crate) fn launch_detected_game() -> Result<(), String> {
    let paths = detect_game_launch_paths()?;
    info!(paks_path = %paths.paks_path.display(), "Launching Marvel Rivals");

    let original_launch_record = backup_launch_record(&paths.launch_record);
    write_skip_launcher_record(&paths.launch_record)?;

    if let Err(e) = launch_via_steam() {
        restore_launch_record(&paths.launch_record, &original_launch_record);
        return Err(e);
    }

    let launch_record = paths.launch_record.clone();
    thread::spawn(move || {
        wait_for_game_start();
        restore_launch_record(&launch_record, &original_launch_record);
    });

    Ok(())
}

fn parent_named<'a>(path: &'a Path, expected_name: &str) -> Result<&'a Path, String> {
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Could not determine parent directory for {}",
            path.display()
        )
    })?;

    if parent
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(expected_name))
    {
        Ok(parent)
    } else {
        Err(format!(
            "Expected {} above {}, found {}",
            expected_name,
            path.display(),
            parent.display()
        ))
    }
}

fn backup_launch_record(path: &Path) -> LaunchRecordBackup {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            info!(path = %path.display(), value = %content.trim(), "Backed up launch_record");
            LaunchRecordBackup::Present(content)
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "launch_record was not readable before launch");
            LaunchRecordBackup::Missing
        }
    }
}

fn write_skip_launcher_record(path: &Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Failed to delete launch_record: {e}"))?;
    }

    std::fs::write(path, "0").map_err(|e| format!("Failed to write launch_record: {e}"))?;
    info!(path = %path.display(), "Temporarily enabled skip launcher");
    Ok(())
}

fn restore_launch_record(path: &Path, backup: &LaunchRecordBackup) {
    if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            warn!(path = %path.display(), error = %e, "Failed to delete launch_record for restore");
            return;
        }
    }

    match backup {
        LaunchRecordBackup::Present(original) => {
            if let Err(e) = std::fs::write(path, original.trim()) {
                warn!(path = %path.display(), error = %e, "Failed to restore launch_record");
            } else {
                info!(path = %path.display(), "Restored launch_record");
            }
        }
        LaunchRecordBackup::Missing => {
            info!(path = %path.display(), "Removed temporary launch_record");
        }
    }
}

fn wait_for_game_start() {
    for _ in 0..30 {
        thread::sleep(Duration::from_secs(1));
        if is_game_process_running() {
            info!("Game process detected; restoring launch_record after short delay");
            thread::sleep(Duration::from_secs(2));
            return;
        }
    }

    warn!("Timed out waiting for game process; restoring launch_record anyway");
}

fn launch_via_steam() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-Command",
                &format!("Start-Process -FilePath '{}'", STEAM_APP_URL),
            ])
            .env("__COMPAT_LAYER", "RUNASINVOKER")
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| {
                error!(error = %e, "Failed to launch Steam URL");
                format!("Failed to launch game. Please ensure Steam is installed. Error: {e}")
            })?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(STEAM_APP_URL)
            .spawn()
            .map_err(|e| format!("Failed to launch game via Steam: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(STEAM_APP_URL)
            .spawn()
            .map_err(|e| format!("Failed to launch game via Steam: {e}"))?;
    }

    info!("Steam launch URL dispatched");
    Ok(())
}

fn is_game_process_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let output = Command::new("tasklist")
            .args([
                "/FI",
                &format!("IMAGENAME eq {GAME_EXE_NAME}"),
                "/FO",
                "CSV",
                "/NH",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        return output
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .to_lowercase()
                    .contains(&GAME_EXE_NAME.to_lowercase())
            })
            .map_or(false, |running| running);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        return Command::new("pgrep")
            .args(["-f", &GAME_EXE_NAME.to_lowercase()])
            .output()
            .map(|output| output.status.success())
            .map_or(false, |running| running);
    }
}
