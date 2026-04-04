extern crate core;

mod file_table;
mod install_mod;
mod main_ui;
mod utils;

pub mod ios_widget;
mod utoc_utils;
mod welcome;
use crate::install_mod::install_mod_logic::iotoc::convert_directory_to_iostore;
use crate::install_mod::map_to_mods_internal;
use crate::main_ui::RepakModManager;
use crate::utils::SkinEntry;
use eframe::egui::{self, IconData};
use retoc::{action_unpack, ActionUnpack, FGuid};
use semver::Version;
use std::cell::LazyCell;
use std::collections::HashMap;
use std::env::args;
use std::fs::{self, create_dir, File};
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use std::thread;
use tracing::{debug, info, instrument};
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
#[cfg(not(debug_assertions))]
use {rfd::MessageButtons, std::panic::PanicHookInfo};

const ICON: LazyCell<Arc<IconData>> = LazyCell::new(|| {
    let d = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../repak-gui/icons/RepakLogoNonCurveFadedRed-modified.png"
    ))
    .expect("The icon data must be valid");

    Arc::new(d)
});

#[cfg(target_os = "windows")]
fn free_console() -> bool {
    unsafe { FreeConsole() == 0 }
}
#[cfg(target_os = "windows")]
fn is_console() -> bool {
    unsafe {
        let mut buffer = [0u32; 1];
        let count = GetConsoleProcessList(buffer.as_mut_ptr(), 1);
        count != 1
    }
}
#[cfg(target_os = "windows")]
#[link(name = "Kernel32")]
extern "system" {
    fn GetConsoleProcessList(process_list: *mut u32, count: u32) -> u32;
    fn FreeConsole() -> i32;
}

#[cfg(target_os = "windows")]
#[cfg(not(debug_assertions))]
fn custom_panic(_info: &PanicHookInfo) -> ! {
    let msg = format!(
        "Repak has crashed. Please report this issue to the developer with the following information:\
\n\n{}\
\nAdditonally include the log file in the bug report"
        ,_info);

    let _x = rfd::MessageDialog::new()
        .set_title("Repak has crashed")
        .set_buttons(MessageButtons::Ok)
        .set_description(msg)
        .show();
    std::process::exit(1);
}

#[instrument(name = "fetch_skins_in_background")]
pub fn fetch_skins_in_background(
) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    info!("Fetching updated skin data");
    thread::spawn(|| {
        let client = reqwest::blocking::Client::new();
        let response = client
            .get("https://raw.githubusercontent.com/donutman07/MarvelRivalsCharacterIDs/refs/heads/main/MarvelRivalsCharacterIDs.md")
            .send()?
            .error_for_status()?;
        let body = response.text()?;
        debug!("Received markdown response ({} bytes)", body.len());

        let skins = parse_markdown_to_skin_entries(&body);
        debug!("Parsed {} skin entries", skins.len());

        let json = serde_json::to_string_pretty(&skins)?;
        fs::create_dir_all("data")?;
        fs::write("data/character_data.json", json)?;
        Ok(())
    })
}

#[instrument(skip(markdown))]
fn parse_markdown_to_skin_entries(markdown: &str) -> Vec<SkinEntry> {
    let mut entries = Vec::new();
    let mut current_char_id: Option<String> = None;
    let mut current_char_name: Option<String> = None;

    for line in markdown.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }

        if line.contains("NAME") || line.contains(":--:") {
            continue;
        }

        let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();

        if cols.len() < 5 {
            continue;
        }

        let id_col = cols[1];
        let name_col = cols[2];
        let skin_id_col = cols[3];
        let skin_name_col = cols[4];

        // Update current character if this row has an ID
        if !id_col.is_empty() && id_col != "????" {
            // Validate it's a numeric character ID
            if id_col.chars().all(|c| c.is_ascii_digit()) {
                current_char_id = Some(id_col.to_string());
                current_char_name = Some(name_col.to_string());
            } else {
                current_char_id = None;
                current_char_name = None;
            }
        }

        // Only emit an entry if we have a valid skin ID
        let skin_id = skin_id_col.trim();
        if skin_id.is_empty() || !skin_id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let Some(ref char_id) = current_char_id else {
            continue;
        };
        let char_name = current_char_name.as_deref().unwrap_or("Unknown");
        let skin_name = if skin_name_col.is_empty() {
            // Fall back to character name for the base/default skin
            char_name.to_string()
        } else {
            skin_name_col.to_string()
        };

        entries.push(SkinEntry {
            skinid: skin_id.to_string(),
            skin_name,
            name: char_name.to_string(),
        });

        // Suppress unused variable warning when char_id is only used in the guard
        let _ = char_id;
    }

    entries
}
#[instrument(name = "fetch_mesh_list_in_bg")]
pub fn fetch_mesh_list_in_bg(
) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    thread::spawn(|| {
        let client = reqwest::blocking::Client::new();

        let response = client
            .get("https://rivals.natimerry.com/meshes")
            .send()?
            .error_for_status()?;

        let body = response.text()?;
        let json: Vec<String> = serde_json::from_str(&body)?;

        let file = File::create("mesh_dir_list.txt")?;
        let mut writer = BufWriter::new(file);

        for line in json {
            writeln!(writer, "{}", line)?;
        }

        Ok(())
    })
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
}

#[instrument(skip(current_version), fields(current_version))]
pub fn check_repak_rivals_version(current_version: &str) {
    let client = reqwest::blocking::Client::new();

    let req = client
        .get("https://api.github.com/repos/natimerry/repak-rivals/releases/latest")
        .header("User-Agent", "repak-rivals-version-check")
        .send();

    if let Err(e) = req {
        rfd::MessageDialog::new()
            .set_title("Failed to query for the latest version")
            .set_buttons(rfd::MessageButtons::Ok)
            .set_description(format!(
                "Repak has failed to query the GitHub API to check for the latest version: {e}"
            ))
            .show();
        return;
    }

    let req = req.unwrap();

    let release: GithubRelease =
        serde_json::from_str(&req.text().expect("failed to parse GitHub response"))
            .expect("Failed to get release data");

    let latest = release.tag_name.trim_start_matches('v');

    let latest_version = Version::parse(latest).expect("invalid latest version format");
    let current_version = Version::parse(current_version).expect("invalid current version format");

    if current_version < latest_version {
        panic!(
            "repak-rivals is outdated: current={}, latest={}",
            current_version, latest_version
        );
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    if !is_console() {
        free_console();
    }
    #[cfg(target_os = "windows")]
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(move |info| {
        custom_panic(info.into());
    }));
    #[cfg(not(debug_assertions))]
    check_repak_rivals_version(env!("CARGO_PKG_VERSION"));

    /*
        Custom baked CLI utility for tobi, if the program detects a specific argument passed to it, it does not spaw GUI
    */

    let args = args().collect::<Vec<String>>();
    if args.len() > 1 {
        if args[1] == "--extract" {
            for _file in &args[2..] {
                // create a new directory for unpacking
                let path = PathBuf::from(&_file);
                let root = path.file_stem().unwrap().to_str().unwrap();
                let result_path = path.parent().unwrap().join(root);
                println!("Creating directory: {:?}", &result_path);

                let _ = create_dir(&result_path).expect("Failed to create extraction directory");
                // build an action
                let action: ActionUnpack = ActionUnpack {
                    utoc: PathBuf::from(&_file),
                    output: result_path,
                    verbose: true,
                };

                let mut config = retoc::Config {
                    container_header_version_override: None,
                    ..Default::default()
                };

                let aes_toc = retoc::AesKey::from_str(
                    "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
                )
                .unwrap();

                config.aes_keys.insert(FGuid::default(), aes_toc.clone());
                let config = Arc::new(config);

                action_unpack(action, config).expect("Failed to extract");
            }
            exit(0);
        }
        if args[1] == "--extract-dir" {
            let search_dir = &args[2];

            let mut success_log: HashMap<PathBuf, PathBuf> = HashMap::new();
            let mut failure_log: Vec<(PathBuf, String)> = Vec::new();

            println!("Searching for .utoc files in: {}", search_dir);

            for entry in WalkDir::new(search_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("utoc"))
            {
                let path = entry.path().to_path_buf();
                let file_stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(v) => v,
                    None => {
                        failure_log.push((path.clone(), "Invalid file stem".into()));
                        continue;
                    }
                };

                let parent_dir = match path.parent() {
                    Some(p) => p,
                    None => {
                        failure_log.push((path.clone(), "No parent directory".into()));
                        continue;
                    }
                };

                let result_path = parent_dir.join(format!("unpacked_{}", file_stem));

                println!("\nProcessing: {:?}", path);
                println!("Using directory: {:?}", result_path);

                // Ensure directory exists (do not fail if it already does)
                if let Err(e) = std::fs::create_dir_all(&result_path) {
                    failure_log.push((path.clone(), format!("Directory create failed: {}", e)));
                    continue;
                }

                let action = ActionUnpack {
                    utoc: path.clone(),
                    output: result_path.clone(),
                    verbose: true,
                };

                let mut config = retoc::Config {
                    container_header_version_override: None,
                    ..Default::default()
                };

                let aes_toc = match retoc::AesKey::from_str(
                    "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74",
                ) {
                    Ok(k) => k,
                    Err(e) => {
                        failure_log.push((path.clone(), format!("Invalid AES key: {}", e)));
                        continue;
                    }
                };

                config.aes_keys.insert(FGuid::default(), aes_toc);
                let config = Arc::new(config);

                match action_unpack(action, config) {
                    Ok(_) => {
                        success_log.insert(path.clone(), result_path.clone());
                        println!("Extracted successfully");
                    }
                    Err(e) => {
                        failure_log.push((path.clone(), format!("Extraction failed: {}", e)));
                    }
                }
            }

            println!("\n{}", "=".repeat(64));
            println!("EXTRACTION SUMMARY");
            println!("{}", "=".repeat(64));

            println!("Successful extractions: {}", success_log.len());
            for (src, dst) in &success_log {
                println!("✓ {:?} → {:?}", src, dst);
            }

            println!("\nFailed extractions: {}", failure_log.len());
            for (path, reason) in &failure_log {
                println!("✗ {:?} — {}", path, reason);
            }

            exit(0);
        }

        if args[1] == "--pack" {
            let paths = args[2..]
                .iter()
                .map(|path| PathBuf::from_str(path).unwrap())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            let installable_mods = map_to_mods_internal(&paths);
            for (i, installable) in installable_mods.iter().enumerate() {
                let mod_dir = paths[i].parent().unwrap();
                let count = AtomicI32::new(0);
                convert_directory_to_iostore(
                    &installable,
                    mod_dir.to_path_buf(),
                    paths[i].clone(),
                    &count,
                )
                .expect("Failed to convert directory");
            }
            exit(0);
        }
    }

    // This forces repak gui to use the XWAYLAND backend instead of the wayland as wayland backend is half baked as shit
    // and doesnt support icons and drag and drop
    unsafe {
        #[cfg(target_os = "linux")]
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let log_path = exe_path
        .parent()
        .expect("Failed to get executable directory")
        .join("latest.log");
    let _log_guard = init_tracing(&log_path);

    info!(
        "Logger initialized at {:?}; egui-family targets restricted to info and above",
        log_path
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1366.0, 768.0])
            .with_min_inner_size([1100.0, 650.])
            .with_drag_and_drop(true)
            .with_icon(ICON.clone()),
        ..Default::default()
    };

    // spawn a background thread to get skins
    #[cfg(not(debug_assertions))]
    fetch_skins_in_background();

    #[cfg(not(debug_assertions))]
    fetch_mesh_list_in_bg();
    eframe::run_native(
        "Repak GUI",
        options,
        Box::new(|cc| {
            cc.egui_ctx
                .style_mut(|style| style.visuals.dark_mode = true);
            Ok(Box::new(
                RepakModManager::load(cc).expect("Unable to load config"),
            ))
        }),
    )
    .expect("Unable to spawn windows");
}

fn init_tracing(log_path: &std::path::Path) -> tracing_appender::non_blocking::WorkerGuard {
    let log_directory = log_path.parent().expect("Failed to get log directory");
    let log_filename = log_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("Failed to get log filename");

    let file_appender = tracing_appender::rolling::never(log_directory, log_filename);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let app_filter = Targets::default()
        .with_default(LevelFilter::DEBUG)
        .with_target("egui", LevelFilter::OFF)
        .with_target("eframe", LevelFilter::OFF)
        .with_target("epaint", LevelFilter::OFF);
    let egui_filter = Targets::default()
        .with_default(LevelFilter::OFF)
        .with_target("egui", LevelFilter::INFO)
        .with_target("eframe", LevelFilter::INFO)
        .with_target("epaint", LevelFilter::INFO);

    let terminal_format = fmt::format()
        .compact()
        .with_target(false)
        .with_thread_names(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);
    let file_format = fmt::format()
        .compact()
        .with_ansi(false)
        .with_target(false)
        .with_thread_names(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .event_format(terminal_format.clone())
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(app_filter.clone()),
        )
        .with(
            fmt::layer()
                .event_format(terminal_format)
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(egui_filter.clone()),
        )
        .with(
            fmt::layer()
                .event_format(file_format.clone())
                .with_writer(file_writer.clone())
                .with_ansi(false)
                .with_filter(app_filter),
        )
        .with(
            fmt::layer()
                .event_format(file_format)
                .with_writer(file_writer)
                .with_ansi(false)
                .with_filter(egui_filter),
        )
        .init();

    guard
}
