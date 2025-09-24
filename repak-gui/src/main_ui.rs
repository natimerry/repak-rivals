extern crate core;

use crate::file_table::FileTable;
use crate::install_mod::{
    self, map_dropped_file_to_mods, map_paths_to_mods, InstallableMod, ModInstallRequest, AES_KEY
};
use crate::ios_widget;
use crate::utils::find_marvel_rivals;
use crate::utils::get_current_pak_characteristics;
use crate::utoc_utils::read_utoc;
use crate::welcome::ShowWelcome;
use eframe::egui::{
    self, style::Selection, Align, Align2, Button, Color32, Id, Label, LayerId, Order,
    RichText, ScrollArea, Stroke, Style, TextEdit, TextStyle, Theme,
};
use egui_flex::{item, Flex, FlexAlign};
use install_mod::install_mod_logic::pak_files::extract_pak_to_dir;
use log::{debug, error, info, trace, warn};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use path_clean::PathClean;
use repak::PakReader;
use rfd::{FileDialog, MessageButtons};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use std::{fs, thread};
use walkdir::WalkDir;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// use eframe::egui::WidgetText::RichText;
#[derive(Deserialize, Serialize, Default)]
pub struct RepakModManager {
    game_path: PathBuf,
    default_font_size: f32,
    #[serde(skip)]
    current_pak_file_idx: Option<usize>,
    #[serde(skip)]
    pak_files: Vec<ModEntry>,
    #[serde(skip)]
    table: Option<FileTable>,
    #[serde(skip)]
    file_drop_viewport_open: bool,
    #[serde(skip)]
    install_mod_dialog: Option<ModInstallRequest>,
    #[serde(skip)]
    receiver: Option<Receiver<Event>>,

    #[serde(skip)]
    welcome_screen: Option<ShowWelcome>,

    #[serde(skip)]
    hide_welcome: bool,
    version: Option<String>,
}

#[derive(Clone)]
struct ModEntry {
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
            version: Some(VERSION.to_string()),
            ..Default::default()
        };
        set_custom_font_size(&cc.egui_ctx, x.default_font_size);
        x
    }

    fn collect_pak_files(&mut self) {
        if self.game_path.exists() {
            let mut vecs = vec![];

            for entry in WalkDir::new(&self.game_path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                if path.is_dir() {
                    continue;
                }
                let mut disabled = false;

                if path.extension().unwrap_or_default() != "pak" {
                    // left in old file extension for compatibility reason
                    if path.extension().unwrap_or_default() == "pak_disabled"
                        || path.extension().unwrap_or_default() == "bak_repak"
                    {
                        disabled = true;
                    } else {
                        continue;
                    }
                }

                let mut builder = repak::PakBuilder::new();
                builder = builder.key(AES_KEY.clone().0);
                let pak = builder.reader(&mut BufReader::new(File::open(path).unwrap()));

                if let Err(_e) = pak {
                    warn!("Error opening pak file");
                    continue;
                }
                let pak = pak.unwrap();
                let entry = ModEntry {
                    reader: pak,
                    path: path.to_path_buf(),
                    enabled: !disabled,
                };
                vecs.push(entry);
            }
            self.pak_files = vecs;
        }
    }
    fn list_pak_contents(&mut self, ui: &mut egui::Ui) -> Result<(), repak::Error> {
        ui.label("Files");
        ui.separator();
        let ctx = ui.ctx();
        self.preview_files_being_dropped(ctx, ui.available_rect_before_wrap());

        // if files are being dropped
        if self.current_pak_file_idx.is_none() && ctx.input(|i| i.raw.hovered_files.is_empty()) {
            let rect = ui.available_rect_before_wrap();
            let painter =
                ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

            let color = ui.style().visuals.faint_bg_color;
            painter.rect_filled(rect, 0.0, color);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Drop .pak files or mod folders here",
                TextStyle::Heading.resolve(&ctx.style()),
                Color32::WHITE,
            );
        }
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
            let mut utoc_path = pak_path.to_path_buf();
            utoc_path.set_extension("utoc");

            let paths = {
                if utoc_path.exists() {
                    let file = read_utoc(&utoc_path, pak, &pak_path)
                        .iter()
                        .map(|entry| entry.file_path.clone())
                        .collect::<Vec<_>>();
                    file
                } else {
                    full_paths.clone()
                }
            };

            ui.add(Label::new(get_current_pak_characteristics(paths)));
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
                            ui.with_layout(egui::Layout::left_to_right(Align::LEFT), |ui| {
                                ui.set_max_width(ui.available_width() * 0.85);
                                let pak_print = pak_file
                                    .path
                                    .file_stem()
                                    .unwrap()
                                    .to_string_lossy()
                                    .to_string();

                                let color = if self.current_pak_file_idx == Some(i) {
                                    Color32::from_hex("#f71034").unwrap()
                                } else {
                                    ui.style().visuals.faint_bg_color
                                };
                                let pakfile = ui.add(
                                    Label::new(
                                        RichText::new(pak_print).strong().background_color(color),
                                    )
                                    .truncate()
                                    .selectable(true),
                                );

                                if pakfile.clicked() {
                                    self.current_pak_file_idx = Some(i);
                                    self.table =
                                        Some(FileTable::new(&pak_file.reader, &pak_file.path));
                                }

                                pakfile.context_menu(|ui| {
                                    if ui.button("Extract pak to directory").clicked(){
                                        self.current_pak_file_idx = Some(i);
                                        let dir = rfd::FileDialog::new().pick_folder();
                                        if let Some(dir) = dir {
                                            let mod_name = pak_file.path.file_stem().unwrap().to_string_lossy().to_string();
                                            let to_create = dir.join(&mod_name);
                                            fs::create_dir_all(&to_create).unwrap();

                                            let installable_mod = InstallableMod{
                                                mod_name: mod_name.clone(),
                                                mod_type: "".to_string(),
                                                reader: Option::from(pak_file.reader.clone()),
                                                mod_path: pak_file.path.clone(),
                                                ..Default::default()
                                            };
                                            if let Err(e) = extract_pak_to_dir(&installable_mod,to_create){
                                                error!("Failed to extract pak directory: {}",e);
                                            }
                                        }
                                    }
                                    if ui.button("Delete mod").clicked(){
                                        let pak_path = &pak_file.path.clone();
                                        let utoc_path = pak_path.with_extension("utoc");
                                        let ucas_path  = pak_path.with_extension("ucas");

                                        let files_to_delete = vec![pak_path, &utoc_path, &ucas_path];

                                        for i in files_to_delete {
                                            let delete_res = fs::remove_file(i);
                                            if let Err(e)  = delete_res {
                                                error!("Failed to delete pak: {}",e);
                                                return;
                                            }
                                        }
                                        self.current_pak_file_idx = None;
                                    }
                                });
                            });

                            // I think we can keep utoc and ucas uncommented as they are only loaded if a valid pak file is present
                            ui.with_layout(egui::Layout::right_to_left(Align::RIGHT), |ui| {
                                let toggler = ui.add(ios_widget::toggle(&mut pak_file.enabled));
                                if toggler.clicked() {
                                    pak_file.enabled = !pak_file.enabled;
                                    if pak_file.enabled {
                                        let new_pak = &pak_file.path.with_extension("bak_repak");
                                        info!("Enabling pak file: {:?}", new_pak);
                                        match std::fs::rename(&pak_file.path, new_pak) {
                                            Ok(_) => {
                                                info!("Renamed pak file: {:?}", new_pak);
                                            }
                                            Err(e) => {
                                                warn!("Failed to rename pak file: {:?}", e);
                                                rfd::MessageDialog::new()
                                                    .set_buttons(MessageButtons::Ok)
                                                    .set_title("Failed to toggle mod")
                                                    .set_description("Failed to rename pak file. Make sure game is not running.")
                                                    .show();
                                            }
                                        }
                                    } else {
                                        let new_pak = &pak_file.path.with_extension("pak");
                                        debug!("Disabling pak file: {:?}", pak_file.path);
                                        match std::fs::rename(&pak_file.path, new_pak) {
                                            Ok(_) => {
                                                info!("Renamed pak file: {:?}", new_pak);
                                            }
                                            Err(e) => {
                                                warn!("Failed to rename pak file: {:?}", e);
                                                rfd::MessageDialog::new()
                                                    .set_buttons(MessageButtons::Ok)
                                                    .set_title("Failed to toggle mod")
                                                    .set_description("Failed to rename pak file. Make sure game is not running.")
                                                    .show();
                                            }
                                        }
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

        debug!("Config path: {}", path.to_string_lossy());
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
            info!("Created config directory: {}", path.to_string_lossy());
        }

        path.push("repak_mod_manager.json");

        path
    }

    pub fn load(ctx: &eframe::CreationContext) -> std::io::Result<Self> {
        let (tx, rx) = channel();
        let path = Self::config_path();
        let mut shit = if path.exists() {
            info!("Loading config: {}", path.to_string_lossy());
            let data = fs::read_to_string(path)?;
            let mut config: Self = serde_json::from_str(&data)?;

            debug!("Setting custom style");
            setup_custom_style(&ctx.egui_ctx);
            debug!("Setting font size: {}", config.default_font_size);
            set_custom_font_size(&ctx.egui_ctx, config.default_font_size);

            info!("Loading mods: {}", config.game_path.to_string_lossy());
            config.collect_pak_files();

            let mut show_welcome = false;
            if let Some(ref version) = config.version {
                if version != VERSION {
                    show_welcome = true;
                }
            } else {
                show_welcome = true;
            }
            config.version = Option::from(VERSION.to_string());
            config.hide_welcome = !show_welcome;
            config.welcome_screen = Some(ShowWelcome{});
            config.receiver = Some(rx);

            Ok(config)
        } else {
            info!(
                "First Launch creating new directory: {}",
                path.to_string_lossy()
            );
            let mut x = Self::new(ctx);
            x.welcome_screen = Some(ShowWelcome{});
            x.hide_welcome=false;
            x.receiver = Some(rx);
            Ok(x)
        };

        if let Ok(ref mut shit) = shit {
            let path = shit.game_path.clone();
            thread::spawn(move || {
                let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
                    if let Ok(event) = res {
                        tx.send(event).unwrap();
                    }
                })
                .unwrap();

                if path.exists() {
                    watcher.watch(&path, RecursiveMode::Recursive).unwrap();
                }

                // Keep the thread alive
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            });
            shit.collect_pak_files();
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
                    path.is_dir()
                        || path
                            .extension()
                            .map(|ext| ext == "pak" || ext == "zip" || ext == "rar")
                            .unwrap_or(false)
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

                    if let Some(dialog) = &self.install_mod_dialog {
                        trace!("Installing mod: {:#?}", dialog.mods);
                    }
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

            if ui.button("Donate").clicked() {
                self.hide_welcome = false;
            }
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
                            let process = std::process::Command::new("explorer.exe")
                                .arg(self.game_path.clone())
                                .spawn();

                            if let Err(e) = process {
                                error!("Failed to open folder: {}", e);
                                return;
                            } else {
                                info!("Opened mod folder: {}", self.game_path.to_string_lossy());
                            }
                            process.unwrap().wait().unwrap();
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
        if let Some(ref mut welcome) = self.welcome_screen{
            if !self.hide_welcome{
                welcome.welcome_screen(ctx,&mut self.hide_welcome);
            }
        }

        let mut collect_pak = false;

        if !self.file_drop_viewport_open && self.install_mod_dialog.is_some() {
            self.install_mod_dialog = None;
        }

        if self.install_mod_dialog.is_none() {
            if let Some(ref receiver) = &self.receiver {
                while let Ok(event) = receiver.try_recv() {
                    match event.kind {
                        EventKind::Any => {
                            warn!("Unknown event received")
                        }
                        EventKind::Other => {}
                        _ => {
                            collect_pak = true;
                        }
                    }
                }
            }
        }
        // if install_mod_dialog is open we dont want to listen to events

        if collect_pak {
            trace!("Collecting pak files");
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