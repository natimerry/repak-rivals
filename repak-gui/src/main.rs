mod file_table;
mod install_mod;
mod pak_logic;
mod utils;

pub mod ios_widget;

use crate::utils::find_marvel_rivals;
use crate::file_table::FileTable;
use crate::install_mod::{map_dropped_file_to_mods, map_paths_to_mods, ModInstallRequest, AES_KEY};
use crate::utils::{ get_current_pak_characteristics};
use eframe::egui::{
    self, style::Selection, Align, Button, Color32, Label, ScrollArea, Stroke, Style, TextEdit,
    TextStyle, Theme,
};
use egui_flex::{item, Flex, FlexAlign};
use log::{debug, error, info, warn};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use repak::PakReader;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use std::{fs, thread};
use path_clean::PathClean;
// use eframe::egui::WidgetText::RichText;

#[derive(Deserialize, Serialize, Default)]
struct RepakModManager {
    game_path: PathBuf,
    default_font_size: f32,
    #[serde(skip)]
    current_pak_file_idx: Option<usize>,
    #[serde(skip)]
    pak_files: Vec<PakEntry>,
    #[serde(skip)]
    table: Option<FileTable>,
    #[serde(skip)]
    file_drop_viewport_open: bool,
    #[serde(skip)]
    install_mod_dialog: Option<ModInstallRequest>,
    #[serde(skip)]
    receiver: Option<Receiver<Event>>,
}

#[derive(Clone)]
struct PakEntry {
    reader: PakReader,
    path: PathBuf,
    enabled: bool,
}
fn use_dark_red_accent(style: &mut Style) {
    style.visuals.hyperlink_color = Color32::from_hex("#f71034").expect("Invalid color");
    style.visuals.text_cursor.stroke.color = Color32::from_hex("#941428").unwrap();
    style.visuals.selection = Selection {
        bg_fill: Color32::from_rgba_unmultiplied(241, 24, 14, 60),
        stroke: Stroke::new(1.0, Color32::from_hex("#000000").unwrap()),
    };

    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(241, 24, 14, 60);
}

pub fn setup_custom_style(ctx: &egui::Context) {
    ctx.style_mut_of(Theme::Dark, use_dark_red_accent);
    ctx.style_mut_of(Theme::Light, use_dark_red_accent);
}

fn set_custom_font_size(ctx: &egui::Context, size: f32) {
    let mut style = (*ctx.style()).clone();
    for (text_style, font_id) in style.text_styles.iter_mut() {
        match text_style {
            TextStyle::Small => {
                font_id.size = size - 4.;
            }
            TextStyle::Body => {
                font_id.size = size - 3.;
            }
            TextStyle::Monospace => {
                font_id.size = size;
            }
            TextStyle::Button => {
                font_id.size = size - 1.;
            }
            TextStyle::Heading => {
                font_id.size = size + 4.;
            }
            TextStyle::Name(_) => {
                font_id.size = size;
            }
        }
    }
    ctx.set_style(style);
}

impl RepakModManager {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let game_install_path = find_marvel_rivals();

        let mut game_path = PathBuf::new();
        if let Some(path) = game_install_path {
            game_path = path.join("~mods").clean();
            fs::create_dir_all(&game_path).unwrap();
        }
        setup_custom_style(&cc.egui_ctx);
        let x = Self {
            game_path,
            default_font_size: 18.0,
            pak_files: vec![],
            current_pak_file_idx: None,
            table: None,
            ..Default::default()
        };
        set_custom_font_size(&cc.egui_ctx, x.default_font_size);
        x
    }

    fn collect_pak_files(&mut self) {
        if !self.game_path.exists() {
        } else {
            let mut vecs = vec![];

            for entry in std::fs::read_dir(self.game_path.clone()).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    continue;
                }
                let mut disabled = false;

                if path.extension().unwrap_or_default() != "pak" {
                    if path.extension().unwrap_or_default() == "pak_disabled" {
                        disabled = true;
                    } else {
                        continue;
                    }
                }

                let mut builder = repak::PakBuilder::new();
                builder = builder.key(AES_KEY.clone().0);
                let pak = builder.reader(&mut BufReader::new(File::open(path.clone()).unwrap()));

                if let Err(e) = pak {
                    error!("Error opening pak file: {}", e);
                    continue;
                }
                let pak = pak.unwrap();
                let entry = PakEntry {
                    reader: pak,
                    path,
                    enabled: !disabled,
                };
                vecs.push(entry);
            }
            self.pak_files = vecs;
        }
    }
    fn list_pak_contents(&mut self, ui: &mut egui::Ui) -> Result<(), repak::Error> {
        if self.current_pak_file_idx.is_none() {
            return Ok(());
        }

        ui.label("Files");
        ui.separator();
        ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let table = &mut self.table;
                if let Some(ref mut table) = table {
                    table.table_ui(ui);
                }
            });
        Ok(())
    }

    fn show_pak_details(&mut self, ui: &mut egui::Ui) {
        if self.current_pak_file_idx.is_none() {
            return;
        }
        use egui::{Label, RichText};
        let pak = &self.pak_files[self.current_pak_file_idx.unwrap()].reader;
        let pak_path = self.pak_files[self.current_pak_file_idx.unwrap()]
            .path
            .clone();
        let full_paths = pak.files().into_iter().collect::<Vec<_>>();

        ui.collapsing("Encryption details", |ui| {
            ui.horizontal(|ui| {
                ui.add(Label::new(RichText::new("Encryption: ").strong()));
                ui.add(Label::new(format!("{}", pak.encrypted_index())));
            });

            ui.horizontal(|ui| {
                ui.add(Label::new(RichText::new("Encryption GUID: ").strong()));
                ui.add(Label::new(format!("{:?}", pak.encryption_guid())));
            });
        });

        ui.collapsing("Pak details", |ui| {
            ui.horizontal(|ui| {
                ui.add(Label::new(RichText::new("Mount Point: ").strong()));
                ui.add(Label::new(pak.mount_point().to_string()));
            });

            ui.horizontal(|ui| {
                ui.add(Label::new(RichText::new("Path Hash Seed: ").strong()));
                ui.add(Label::new(format!("{:?}", pak.path_hash_seed())));
            });

            ui.horizontal(|ui| {
                ui.add(Label::new(RichText::new("Version: ").strong()));
                ui.add(Label::new(format!("{:?}", pak.version())));
            });
        });
        ui.horizontal(|ui| {
            ui.add(Label::new(
                RichText::new("Mod type: ")
                    .strong()
                    .size(self.default_font_size + 1.),
            ));
            ui.add(Label::new(get_current_pak_characteristics(full_paths.clone()).to_string()));
        });
        if self.table.is_none() {
            self.table = Some(FileTable::new(pak, &pak_path));
        }
    }
    fn show_pak_files_in_dir(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for (i, pak_file) in self.pak_files.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if let Some(_idx) = self.current_pak_file_idx {}
                            let pakfile = ui.selectable_label(
                                i == self.current_pak_file_idx.unwrap_or(usize::MAX),
                                pak_file
                                    .path
                                    .file_stem()
                                    .unwrap()
                                    .to_string_lossy()
                                    .to_string(),
                            );
                            if pakfile.clicked() {
                                self.current_pak_file_idx = Some(i);
                            }

                            ui.with_layout(egui::Layout::right_to_left(Align::RIGHT), |ui| {
                                let toggler = ui.add(ios_widget::toggle(&mut pak_file.enabled));
                                if toggler.clicked() {
                                    pak_file.enabled = !pak_file.enabled;
                                    if pak_file.enabled {
                                        let new_pak = &pak_file.path.with_extension("pak_disabled");
                                        info!("Enabling pak file: {:?}", new_pak);
                                        std::fs::rename(&pak_file.path, new_pak)
                                            .expect("Failed to rename pak file");
                                    } else {
                                        let new_pak = &pak_file.path.with_extension("pak");
                                        std::fs::rename(&pak_file.path, new_pak)
                                            .expect("Failed to rename pak file");
                                        let _ = std::fs::rename(&pak_file.path, new_pak);
                                    }
                                }
                            });
                        });
                    }
                });
            });
    }
    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("repak_manager");
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
            info!("Created config directory: {}", path.to_string_lossy());
        }
        path.push("repak_mod_manager.json");
        path
    }

    fn load(ctx: &eframe::CreationContext) -> std::io::Result<Self> {
        let (tx, rx) = channel();

        let path = Self::config_path();
        let shit = if path.exists() {
            info!("Loading config: {}", path.to_string_lossy());
            let data = fs::read_to_string(path)?;
            let mut config: Self = serde_json::from_str(&data)?;

            setup_custom_style(&ctx.egui_ctx);
            set_custom_font_size(&ctx.egui_ctx, config.default_font_size);
            config.collect_pak_files();
            config.receiver = Some(rx);

            Ok(config)
        } else {
            info!(
                "First Launch creating new directory: {}",
                path.to_string_lossy()
            );
            let mut x = Self::new(ctx);
            x.receiver = Some(rx);
            Ok(x)
        };

        if let Ok(ref shit) = shit {
            let path = shit.game_path.clone();
            thread::spawn(move || {
                let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
                    if let Ok(event) = res {
                        tx.send(event).unwrap();
                    }
                })
                .unwrap();

                watcher
                    .watch(&path, RecursiveMode::Recursive)
                    .unwrap();

                // Keep the thread alive
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            });
        }
        shit
    }
    fn save_state(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)?;
        info!("Saving config: {}", path.to_string_lossy());
        fs::write(path, json)?;
        Ok(())
    }

    /// Preview hovering files:
    fn preview_files_being_dropped(&self, ctx: &egui::Context, rect: egui::Rect) {
        use egui::{Align2, Color32, Id, LayerId, Order, TextStyle};

        if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
            let painter =
                ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

            let msg = match self.game_path.is_dir() {
                true => "Drop mod files here",
                false => "Choose a game directory first!!!",
            };
            painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(241, 24, 14, 40));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                msg,
                TextStyle::Heading.resolve(&ctx.style()),
                Color32::WHITE,
            );
        }
    }

    fn check_drop(&mut self, ctx: &egui::Context) {
        if !self.game_path.is_dir() {
            return;
        }
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                let dropped_files = i.raw.dropped_files.clone();
                // Check if all files are either directories or have the .pak extension
                let all_valid = dropped_files.iter().all(|file| {
                    let path = file.path.clone().unwrap();
                    path.is_dir() || path.extension().map(|ext| ext == "pak").unwrap_or(false)
                });

                if all_valid {
                    let mods = map_dropped_file_to_mods(&dropped_files);
                    if mods.is_empty() {
                        error!("No mods found in dropped files.");
                        return;
                    }
                    self.file_drop_viewport_open = true;
                    debug!("Mods: {:?}", mods);
                    self.install_mod_dialog =
                        Some(ModInstallRequest::new(mods, self.game_path.clone()));
                    debug!("Installing mod: {:?}", &self.install_mod_dialog);
                } else {
                    // Handle the case where not all dropped files are valid
                    // You can show an error or prompt the user here
                    println!(
                        "Not all files are valid. Only directories or .pak files are allowed."
                    );
                }
            }
        });
    }

    fn show_menu_bar(&mut self, ui: &mut egui::Ui) -> Result<(), repak::Error> {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                let msg = match self.game_path.is_dir() {
                    true => "Drop mod files here",
                    false => "Choose a game directory first!!!",
                };

                if ui
                    .add_enabled(self.game_path.is_dir(), Button::new("Install mods"))
                    .on_hover_text(msg)
                    .clicked()
                {
                    ui.close_menu(); // Closes the menu
                    let mod_files = rfd::FileDialog::new()
                        .set_title("Pick mods")
                        .pick_files()
                        .unwrap_or_default();

                    if mod_files.is_empty() {
                        error!("No mods found in dropped files.");
                        return;
                    }

                    let mods = map_paths_to_mods(&mod_files);
                    if mods.is_empty() {
                        error!("No mods found in dropped files.");
                        return;
                    }

                    self.file_drop_viewport_open = true;
                    self.install_mod_dialog =
                        Some(ModInstallRequest::new(mods, self.game_path.clone()));
                }

                if ui
                    .add_enabled(self.game_path.is_dir(), Button::new("Pack folder"))
                    .on_hover_text(msg)
                    .clicked()
                {
                    ui.close_menu(); // Closes the menu
                    let mod_files = rfd::FileDialog::new()
                        .set_title("Pick mods")
                        .pick_folders()
                        .unwrap_or_default();

                    if mod_files.is_empty() {
                        error!("No folders picked. Please pick a folder with mods in it.");
                        return;
                    }

                    let mods = map_paths_to_mods(&mod_files);
                    if mods.is_empty() {
                        error!("No mods found in dropped files.");
                        return;
                    }
                    self.file_drop_viewport_open = true;
                    self.install_mod_dialog =
                        Some(ModInstallRequest::new(mods, self.game_path.clone()));
                }
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Settings", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.default_font_size, 12.0..=32.0).text("Font size"),
                );
                set_custom_font_size(ui.ctx(), self.default_font_size);
                ui.horizontal(|ui| {
                    let mode = match ui.ctx().style().visuals.dark_mode {
                        true => "Switch to light mode",
                        false => "Switch to dark mode",
                    };
                    ui.add(egui::Label::new(mode).halign(Align::Center));
                    egui::widgets::global_theme_preference_switch(ui);
                });
            });
        });

        Ok(())
    }

    fn show_file_dialog(&mut self, ui: &mut egui::Ui) {
        Flex::horizontal()
            .w_full()
            .align_items(FlexAlign::Center)
            .show(ui, |flex_ui| {
                flex_ui.add(item(), Label::new("Mod folder:"));
                flex_ui.add(
                    item().grow(1.0),
                    TextEdit::singleline(&mut self.game_path.to_string_lossy().to_string()),
                );
                let browse_button = flex_ui.add(item(), Button::new("Browse"));
                if browse_button.clicked() {
                    if let Some(path) = FileDialog::new().pick_folder() {
                        self.game_path = path;
                    }
                }
                flex_ui.add_ui(item(), |ui| {
                    let x = ui.add_enabled(self.game_path.exists(), Button::new("Open mod folder"));
                    if x.clicked() {
                        println!("Opening mod folder: {}", self.game_path.to_string_lossy());
                        #[cfg(target_os = "windows")]
                        {
                            let _ = std::process::Command::new("explorer")
                                .arg(self.game_path.clone())
                                .spawn();
                        }

                        #[cfg(target_os = "linux")]
                        {
                            debug!("Opening mod folder: {}", self.game_path.to_string_lossy());
                            let _ = std::process::Command::new("xdg-open")
                                .arg(self.game_path.to_string_lossy().to_string())
                                .spawn();
                        }
                    }
                });
            });
    }
}
impl eframe::App for RepakModManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut collect_pak = false;
        if let Some(ref receiver) = &self.receiver {
            while let Ok(event) = receiver.try_recv() {
                match event.kind {
                    EventKind::Any => {
                        warn!("Unknown event received")
                    }
                    EventKind::Other => {}
                    _ => {
                        debug!("Received event {:?}", event.kind);
                        collect_pak = true;
                    }
                }
            }
        }

        if collect_pak {
            info!("Collecting pak files");
            self.collect_pak_files();
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            if let Err(e) = self.show_menu_bar(ui) {
                error!("Error: {}", e);
            }

            ui.separator();
            self.show_file_dialog(ui);
        });

        egui::SidePanel::left("left_panel")
            .min_width(300.)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.set_height(ui.available_height());
                    ui.label("Mod files");
                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height(ui.available_height() * 0.6);
                        self.show_pak_files_in_dir(ui);
                    });

                    ui.separator();

                    ui.label("Details");

                    ui.group(|ui| {
                        ui.set_height(ui.available_height());
                        ui.set_width(ui.available_width());
                        self.show_pak_details(ui);
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.preview_files_being_dropped(ctx, ui.available_rect_before_wrap());
            self.list_pak_contents(ui).expect("TODO: panic message");
        });

        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_state().unwrap();
        }
        self.check_drop(ctx);
        if let Some(ref mut install_mod) = self.install_mod_dialog {
            if self.file_drop_viewport_open {
                install_mod.new_mod_dialog(ctx, &mut self.file_drop_viewport_open);
            }
        }
    }
}

fn main() {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1366.0, 768.0])
            .with_min_inner_size([1280.0, 720.])
            .with_drag_and_drop(true),
        ..Default::default()
    };

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
