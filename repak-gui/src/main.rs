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
use eframe::egui::{self, IconData};
use log::{debug, info, LevelFilter};
use retoc::{action_unpack, ActionUnpack, FGuid};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode, WriteLogger};
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

pub fn fetch_skins_in_background(
) -> thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    thread::spawn(|| {
        let client = reqwest::blocking::Client::new();

        let response = client
            .get("https://rivals.natimerry.com/skins")
            .send()?
            .error_for_status()?;

        let body = response.text()?;
        debug!("Received response: {:?}", body);

        fs::create_dir_all("data")?;
        fs::write("data/character_data.json", body)?;

        Ok(())
    })
}

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

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

pub fn check_repak_rivals_version(current_version: &str) {
    let client = Client::new();

    let release: GithubRelease = client
        .get("https://api.github.com/repos/natimerry/repak-rivals/releases/latest")
        .header("User-Agent", "repak-rivals-version-check")
        .send()
        .expect("failed to query GitHub API")
        .error_for_status()
        .expect("GitHub API returned error")
        .json()
        .expect("failed to parse GitHub response");

    // Strip leading 'v' if present (common GitHub tagging style)
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
    check_repak_rivals_version(env!("CARGO_PKG_VERSION"));

    #[cfg(target_os = "windows")]
    if !is_console() {
        free_console();
    }
    #[cfg(target_os = "windows")]
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(move |info| {
        custom_panic(info.into());
    }));

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

    let log_file = File::create("latest.log").expect("Failed to create log file");
    let level_filter = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
    ])
    .expect("Failed to initialize logger");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1366.0, 768.0])
            .with_min_inner_size([1100.0, 650.])
            .with_drag_and_drop(true)
            .with_icon(ICON.clone()),
        ..Default::default()
    };

    info!("Fetching updated skin data");

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
