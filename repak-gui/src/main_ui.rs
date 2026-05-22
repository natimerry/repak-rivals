extern crate core;

use crate::file_table::FileTable;
use crate::install_mod::install_mod_logic::iotoc::{to_legacy_uasset, to_legacy_uasset_fast};
use crate::install_mod::{
    self, map_dropped_file_to_mods, map_paths_to_mods, InstallableMod, ModInstallRequest, AES_KEY,
};
use crate::ios_widget;
use crate::utils::{
    find_marvel_rivals, get_current_pak_characteristics, latest_depot_usmap_path,
    match_exact_paks_suffix, mods_need_kawaii_mapping,
};
use crate::utoc_utils::{is_iostore_obfuscated, read_utoc_package_names};
use crate::welcome::ShowWelcome;

use eframe::egui::{
    self, style::Selection, Align, Align2, Button, Color32, Id, Label, LayerId, Order, RichText,
    ScrollArea, Stroke, Style, TextEdit, TextStyle, Theme,
};
use egui_flex::{item, Flex, FlexAlign};
use install_mod::install_mod_logic::fix_installed_iostore_kawaii_physics;
use install_mod::install_mod_logic::pak_files::extract_pak_to_dir;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use path_clean::PathClean;
use repak::PakReader;
use rfd::{FileDialog, MessageButtons};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, instrument, trace, warn};
use walkdir::WalkDir;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const RED_THEME_COLOR: Color32 = Color32::from_rgb(255, 31, 75);
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
    selected_pak_details: Option<SelectedPakDetailsState>,
    #[serde(skip)]
    file_drop_viewport_open: bool,
    #[serde(skip)]
    install_mod_dialog: Option<ModInstallRequest>,
    #[serde(skip)]
    receiver: Option<Receiver<Event>>,
    #[serde(skip)]
    watcher_tx: Option<Sender<Event>>,
    #[serde(skip)]
    watcher: Option<RecommendedWatcher>,
    #[serde(skip)]
    metadata_receiver: Option<Receiver<MetadataMessage>>,
    #[serde(skip)]
    metadata_cancel: Option<Arc<AtomicBool>>,
    #[serde(skip)]
    metadata_generation: u64,
    #[serde(skip)]
    pending_refresh_at: Option<Instant>,
    #[serde(skip)]
    metadata_cache_dirty: bool,

    #[serde(skip)]
    welcome_screen: Option<ShowWelcome>,

    #[serde(skip)]
    hide_welcome: bool,
    #[serde(skip)]
    mod_files_search_query: String,
    #[serde(skip)]
    selected_tag_filters: Vec<String>,
    #[serde(skip)]
    selected_category_filters: Vec<String>,
    #[serde(skip)]
    new_tag_name: String,
    version: Option<String>,

    game_chunk_path: Option<PathBuf>,
    #[serde(default)]
    kawaii_physics_usmap: Option<PathBuf>,
    #[serde(skip)]
    launch_game_paths: Option<Result<crate::launch_game::GameLaunchPaths, String>>,
    #[serde(default)]
    tag_catalog: Vec<String>,
    #[serde(default)]
    mod_tags: Vec<ModTagAssignment>,
    #[serde(default)]
    mod_metadata_cache: Vec<ModMetadataCacheEntry>,

    #[serde(default)]
    show_load_order_suffix: bool,

    #[serde(default = "default_true")]
    show_char_details: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Deserialize, Serialize)]
struct ModTagAssignment {
    path: PathBuf,
    tags: Vec<String>,
}

#[derive(Clone)]
struct ModEntry {
    path: PathBuf,
    enabled: bool,
    is_iostore: bool,
    signature: ModFileSignature,
    category: String,
    characteristic: String,
    obfuscated: String,
    metadata_pending: bool,
    file_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ModFileSignature {
    size: u64,
    modified_secs: u64,
}

#[derive(Clone, Deserialize, Serialize)]
struct ModMetadataCacheEntry {
    identity: String,
    signature: ModFileSignature,
    category: String,
    characteristic: String,
    obfuscated: String,
    file_count: Option<usize>,
}

struct MetadataJob {
    generation: u64,
    path: PathBuf,
    identity: String,
    signature: ModFileSignature,
    obfuscated: String,
    is_iostore: bool,
}

struct MetadataResult {
    generation: u64,
    identity: String,
    signature: ModFileSignature,
    category: String,
    characteristic: String,
    obfuscated: String,
    file_count: Option<usize>,
}

enum MetadataMessage {
    Entry(MetadataResult),
    Done(u64),
}

struct SelectedPakDetails {
    path: PathBuf,
    mount_point: String,
    path_hash_seed: String,
    version: String,
}

enum SelectedPakDetailsState {
    Loaded(SelectedPakDetails),
    Failed { path: PathBuf, error: String },
}

fn use_dark_red_accent(style: &mut Style) {
    style.visuals.hyperlink_color = Color32::from_hex("#f71034").expect("Invalid color");
    style.visuals.text_cursor.stroke.color = Color32::from_hex("#941428").unwrap();
    style.visuals.selection = Selection {
        bg_fill: Color32::from_rgba_unmultiplied(241, 24, 14, 60),
        stroke: Stroke::new(1.0_f32, Color32::from_hex("#000000").unwrap()),
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
    #[instrument(skip(cc))]
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let game_install_path = find_marvel_rivals();

        let mut game_path = PathBuf::new();
        if let Some(path) = game_install_path {
            game_path = path.join("~mods").clean();
            fs::create_dir_all(&game_path).unwrap();
        }
        setup_custom_style(&cc.egui_ctx);
        let mut x = Self {
            game_path,
            default_font_size: 18.0,
            pak_files: vec![],
            current_pak_file_idx: None,
            table: None,
            version: Some(VERSION.to_string()),
            ..Default::default()
        };
        x.set_game_pakchunk_path();
        set_custom_font_size(&cc.egui_ctx, x.default_font_size);
        x
    }

    fn set_game_pakchunk_path(&mut self) {
        self.game_chunk_path = None;
        let Some(path) = self.game_path.parent() else {
            return;
        };

        if let Some(paks_path) = match_exact_paks_suffix(path) {
            self.game_chunk_path = Some(paks_path)
        }
    }

    fn collect_pak_files(&mut self) {
        info!("Refreshing mods with fast scan");
        if !self.game_path.exists() {
            self.pak_files.clear();
            self.current_pak_file_idx = None;
            self.table = None;
            self.selected_pak_details = None;
            return;
        }

        self.metadata_generation = self.metadata_generation.wrapping_add(1);
        let generation = self.metadata_generation;
        let selected_identity = self
            .current_pak_file_idx
            .and_then(|idx| self.pak_files.get(idx))
            .map(|entry| normalized_mod_identity_string(&entry.path));
        let cache_by_identity = self
            .mod_metadata_cache
            .iter()
            .map(|entry| (entry.identity.clone(), entry.clone()))
            .collect::<HashMap<_, _>>();
        let mut next_entries = Vec::new();
        let mut jobs = Vec::new();

        for entry in WalkDir::new(&self.game_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            let Some((enabled, is_mod_file)) = mod_file_state(path) else {
                continue;
            };
            if !is_mod_file {
                continue;
            }

            let Some(signature) = mod_file_signature(path) else {
                warn!(
                    file_name = %path.file_name().and_then(|name| name.to_str()).unwrap_or("<unknown>"),
                    "Skipping mod with unreadable metadata"
                );
                continue;
            };
            let identity = normalized_mod_identity_string(path);
            let utoc_path = path.with_extension("utoc");
            let is_iostore = utoc_path.exists();
            let obfuscated = if is_iostore {
                match is_iostore_obfuscated(&utoc_path) {
                    Ok(value) => value.to_string(),
                    Err(e) => {
                        warn!(error = %e, "Failed to read IoStore obfuscation flag");
                        "Unknown".to_string()
                    }
                }
            } else {
                "false".to_string()
            };

            let cached = cache_by_identity
                .get(&identity)
                .filter(|entry| entry.signature == signature);
            let (category, characteristic, metadata_pending, file_count) = match cached {
                Some(entry) => (
                    entry.category.clone(),
                    entry.characteristic.clone(),
                    false,
                    entry.file_count,
                ),
                None => {
                    jobs.push(MetadataJob {
                        generation,
                        path: path.to_path_buf(),
                        identity: identity.clone(),
                        signature,
                        obfuscated: obfuscated.clone(),
                        is_iostore,
                    });
                    ("Pending".to_string(), "Pending".to_string(), true, None)
                }
            };

            next_entries.push(ModEntry {
                path: path.to_path_buf(),
                enabled,
                is_iostore,
                signature,
                category,
                characteristic,
                obfuscated,
                metadata_pending,
                file_count,
            });
        }

        self.pak_files = next_entries;
        self.current_pak_file_idx = selected_identity.and_then(|identity| {
            self.pak_files
                .iter()
                .position(|entry| normalized_mod_identity_string(&entry.path) == identity)
        });
        if self.current_pak_file_idx.is_none() {
            self.table = None;
            self.selected_pak_details = None;
        }

        self.start_metadata_worker(generation, jobs);
        self.prune_metadata_cache();
        info!(
            mod_entries = self.pak_files.len(),
            "Loaded mod entries from fast scan"
        );
    }

    fn start_metadata_worker(&mut self, generation: u64, jobs: Vec<MetadataJob>) {
        if let Some(cancel) = &self.metadata_cancel {
            cancel.store(true, Ordering::Relaxed);
        }

        if jobs.is_empty() {
            self.metadata_receiver = None;
            self.metadata_cancel = None;
            return;
        }

        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.metadata_receiver = Some(rx);
        self.metadata_cancel = Some(cancel.clone());
        std::thread::spawn(move || {
            for job in jobs {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match classify_mod_metadata(&job) {
                    Ok(result) => {
                        if tx.send(MetadataMessage::Entry(result)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to classify mod metadata");
                    }
                }
            }
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send(MetadataMessage::Done(generation));
            }
        });
    }

    fn process_metadata_messages(&mut self, ctx: &egui::Context) {
        let mut finished_generation = None;
        let mut received_entry = false;
        let mut messages = Vec::new();

        if let Some(receiver) = &self.metadata_receiver {
            while let Ok(message) = receiver.try_recv() {
                messages.push(message);
            }
        }

        for message in messages {
            match message {
                MetadataMessage::Entry(result) => {
                    if result.generation != self.metadata_generation {
                        continue;
                    }
                    self.apply_metadata_result(result);
                    received_entry = true;
                }
                MetadataMessage::Done(generation) => {
                    if generation == self.metadata_generation {
                        finished_generation = Some(generation);
                    }
                }
            }
        }

        if received_entry {
            ctx.request_repaint();
        }

        if finished_generation.is_some() {
            self.metadata_receiver = None;
            self.metadata_cancel = None;
            if self.metadata_cache_dirty {
                if let Err(e) = self.save_state() {
                    warn!(error = %e, "Failed to save metadata cache");
                }
                self.metadata_cache_dirty = false;
            }
        }
    }

    fn apply_metadata_result(&mut self, result: MetadataResult) {
        if let Some(entry) = self
            .pak_files
            .iter_mut()
            .find(|entry| normalized_mod_identity_string(&entry.path) == result.identity)
        {
            if entry.signature != result.signature {
                return;
            }
            entry.category = result.category.clone();
            entry.characteristic = result.characteristic.clone();
            entry.obfuscated = result.obfuscated.clone();
            entry.metadata_pending = false;
            entry.file_count = result.file_count;
        }

        let cache_entry = ModMetadataCacheEntry {
            identity: result.identity.clone(),
            signature: result.signature,
            category: result.category,
            characteristic: result.characteristic,
            obfuscated: result.obfuscated,
            file_count: result.file_count,
        };

        if let Some(existing) = self
            .mod_metadata_cache
            .iter_mut()
            .find(|entry| entry.identity == result.identity)
        {
            *existing = cache_entry;
        } else {
            self.mod_metadata_cache.push(cache_entry);
        }
        self.metadata_cache_dirty = true;
    }

    fn prune_metadata_cache(&mut self) {
        let active = self
            .pak_files
            .iter()
            .map(|entry| normalized_mod_identity_string(&entry.path))
            .collect::<std::collections::HashSet<_>>();
        let before = self.mod_metadata_cache.len();
        self.mod_metadata_cache
            .retain(|entry| active.contains(&entry.identity));
        if self.mod_metadata_cache.len() != before {
            self.metadata_cache_dirty = true;
        }
    }

    fn open_pak_reader(pak_path: &Path) -> Result<PakReader, String> {
        let mut builder = repak::PakBuilder::new();
        builder = builder.key(AES_KEY.clone().0);
        let file = File::open(pak_path)
            .map_err(|e| format!("Failed to open {}: {e}", pak_path.display()))?;
        builder
            .reader(&mut BufReader::new(file))
            .map_err(|e| format!("Failed to read {}: {e}", pak_path.display()))
    }

    fn select_mod(&mut self, idx: usize) {
        let Some(entry) = self.pak_files.get(idx) else {
            return;
        };
        let pak_path = entry.path.clone();
        self.current_pak_file_idx = Some(idx);
        match Self::open_pak_reader(&pak_path) {
            Ok(reader) => {
                self.selected_pak_details =
                    Some(SelectedPakDetailsState::Loaded(SelectedPakDetails {
                        path: pak_path.clone(),
                        mount_point: reader.mount_point().to_string(),
                        path_hash_seed: format!("{:?}", reader.path_hash_seed()),
                        version: format!("{:?}", reader.version()),
                    }));
                self.table = Some(FileTable::new(&reader, &pak_path));
            }
            Err(error) => {
                self.selected_pak_details = Some(SelectedPakDetailsState::Failed {
                    path: pak_path,
                    error,
                });
                self.table = None;
            }
        }
    }

    #[instrument(skip(self), fields(game_path_exists = self.game_path.exists()))]
    fn restart_game_path_watcher(&mut self) {
        self.watcher = None;
        let Some(tx) = self.watcher_tx.clone() else {
            warn!("Watcher sender was not initialized; skipping watcher setup");
            return;
        };

        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        }) {
            Ok(watcher) => watcher,
            Err(e) => {
                warn!(error = %e, "Failed to create filesystem watcher");
                return;
            }
        };

        if self.game_path.exists() {
            if let Err(e) = watcher.watch(&self.game_path, RecursiveMode::Recursive) {
                warn!(error = %e, "Failed to watch game path");
                return;
            }
            info!("Watching game path for changes");
        } else {
            warn!("Game path does not exist; watcher not started");
        }

        self.watcher = Some(watcher);
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
        let Some(current_idx) = self.current_pak_file_idx else {
            return;
        };
        let Some(current_mod) = self.pak_files.get(current_idx) else {
            return;
        };
        let obfuscated = current_mod.obfuscated.clone();
        let characteristic = current_mod.characteristic.clone();

        egui::CollapsingHeader::new("Pak details")
            .default_open(true)
            .show(ui, |ui| {
                match &self.selected_pak_details {
                    Some(SelectedPakDetailsState::Loaded(details))
                        if same_mod_identity(&details.path, &current_mod.path) =>
                    {
                        ui.horizontal(|ui| {
                            ui.add(Label::new(RichText::new("Mount Point: ").strong()));
                            ui.add(Label::new(details.mount_point.clone()));
                        });

                        ui.horizontal(|ui| {
                            ui.add(Label::new(RichText::new("Path Hash Seed: ").strong()));
                            ui.add(Label::new(details.path_hash_seed.clone()));
                        });

                        ui.horizontal(|ui| {
                            ui.add(Label::new(RichText::new("Version: ").strong()));
                            ui.add(Label::new(details.version.clone()));
                        });
                    }
                    Some(SelectedPakDetailsState::Failed { path, error })
                        if same_mod_identity(path, &current_mod.path) =>
                    {
                        ui.horizontal(|ui| {
                            ui.add(Label::new(RichText::new("Pak reader: ").strong()));
                            ui.add(Label::new(error.clone()));
                        });
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.add(Label::new(RichText::new("Pak reader: ").strong()));
                            ui.add(Label::new("Select the mod again to load details"));
                        });
                    }
                }

                ui.horizontal(|ui| {
                    ui.add(Label::new(RichText::new("Obfuscated: ").strong()));
                    ui.add(Label::new(obfuscated));
                });
            });
        ui.horizontal(|ui| {
            ui.add(Label::new(
                RichText::new("Mod type: ")
                    .strong()
                    .size(self.default_font_size + 1.),
            ));
            ui.add(Label::new(characteristic));
        });
    }
    #[instrument(skip(self, ui))]
    fn show_pak_files_in_dir(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut self.mod_files_search_query)
                                .hint_text("Search mods, paths, tags...")
                                .desired_width(230.0),
                        );
                        egui::ComboBox::from_id_salt("category_filter")
                            .width(145.0)
                            .selected_text(filter_label(
                                "Category",
                                &self.selected_category_filters,
                            ))
                            .show_ui(ui, |ui| {
                                if ui
                                    .button(if self.selected_category_filters.is_empty() {
                                        "All categories"
                                    } else {
                                        "Clear categories"
                                    })
                                    .clicked()
                                {
                                    self.selected_category_filters.clear();
                                }
                                ui.separator();
                                for category in self.category_options() {
                                    let mut selected =
                                        self.selected_category_filters.contains(&category);
                                    if ui.checkbox(&mut selected, &category).changed() {
                                        toggle_filter_value(
                                            &mut self.selected_category_filters,
                                            category,
                                            selected,
                                        );
                                    }
                                }
                            });

                        egui::ComboBox::from_id_salt("tag_filter")
                            .width(135.0)
                            .selected_text(filter_label("Tag", &self.selected_tag_filters))
                            .show_ui(ui, |ui| {
                                if ui
                                    .button(if self.selected_tag_filters.is_empty() {
                                        "All tags"
                                    } else {
                                        "Clear tags"
                                    })
                                    .clicked()
                                {
                                    self.selected_tag_filters.clear();
                                }
                                ui.separator();
                                for tag in self.tag_catalog.clone() {
                                    let mut selected = self.selected_tag_filters.contains(&tag);
                                    if ui.checkbox(&mut selected, &tag).changed() {
                                        toggle_filter_value(
                                            &mut self.selected_tag_filters,
                                            tag,
                                            selected,
                                        );
                                    }
                                }
                            });

                        let has_filters = !self.mod_files_search_query.is_empty()
                            || !self.selected_category_filters.is_empty()
                            || !self.selected_tag_filters.is_empty();
                        if ui.add_enabled(has_filters, Button::new("Reset")).clicked() {
                            self.mod_files_search_query.clear();
                            self.selected_category_filters.clear();
                            self.selected_tag_filters.clear();
                        }
                    });
                    ui.add_space(8.0);

                    for i in 0..self.pak_files.len() {
                        let search_query = self.mod_files_search_query.trim().to_lowercase();
                        let pak_path = self.pak_files[i].path.clone();
                        let pak_category = self.pak_files[i].category.clone();
                        let pak_enabled = self.pak_files[i].enabled;
                        let raw_name = pak_path
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .unwrap_or_default()
                            .to_string();
                        let display_name = normalize_mod_display_name(&raw_name.clone());
                        let has_suffix_indicator = has_mod_suffix(&raw_name);
                        let raw_path = pak_path.to_string_lossy().to_string();
                        let tag_list = self.tags_for_mod(&pak_path);
                        if !search_query.is_empty()
                            && !raw_name.to_lowercase().contains(&search_query)
                            && !display_name.to_lowercase().contains(&search_query)
                            && !raw_path.to_lowercase().contains(&search_query)
                            && !tag_list.iter().any(|tag| tag.to_lowercase().contains(&search_query))
                        {
                            continue;
                        }
                        if !self.selected_category_filters.is_empty()
                            && !self.selected_category_filters.contains(&pak_category)
                        {
                            continue;
                        }
                        if !self.selected_tag_filters.is_empty()
                            && !tag_list
                                .iter()
                                .any(|tag| self.selected_tag_filters.contains(tag))
                        {
                            continue;
                        }

                        let is_iostore = self.pak_files[i].is_iostore;
                        let is_selected = self.current_pak_file_idx == Some(i);
                        let row_fill = if is_selected {
                            Color32::from_rgb(54, 28, 35)
                        } else {
                            Color32::from_rgb(35, 35, 35)
                        };
                        let row_stroke = if is_selected {
                            Stroke::new(1.0_f32, RED_THEME_COLOR)
                        } else {
                            Stroke::new(1.0_f32, Color32::from_rgb(64, 64, 64))
                        };

                        let mut toggler_clicked = false;
                        let row = egui::Frame::NONE
                            .fill(row_fill)
                            .stroke(row_stroke)
                            .corner_radius(4)
                            .inner_margin(egui::Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let left_width = (ui.available_width() - 62.0).max(120.0);
                                    ui.vertical(|ui| {
                                        ui.set_width(left_width);
                                        ui.add(
                                            Label::new(
                                                RichText::new(&display_name)
                                                    .strong()
                                                    .size(self.default_font_size),
                                            )
                                            .truncate()
                                            .selectable(false),
                                        );

                                        ui.add_space(2.0);
                                        ui.horizontal_wrapped(|ui| {
                                            if has_suffix_indicator && self.show_load_order_suffix{
                                                self.metadata_chip(
                                                    ui,
                                                    "_9999999_P",
                                                    Color32::from_rgb(180, 180, 180),
                                                    Color32::from_rgb(58, 58, 58),
                                                    Color32::from_rgb(82, 82, 82),
                                                );
                                            }
                                            let (category_text, category_fill, category_stroke) =
                                                category_colors(&pak_category);
                                            self.metadata_chip(
                                                ui,
                                                &pak_category,
                                                category_text,
                                                category_fill,
                                                category_stroke,
                                            );
                                            for tag in &tag_list {
                                                let (tag_text, tag_fill, tag_stroke) =
                                                    tag_colors(tag);
                                                self.metadata_chip(
                                                    ui,
                                                    &format!("#{tag}"),
                                                    tag_text,
                                                    tag_fill,
                                                    tag_stroke,
                                                );
                                            }
                                            if self.show_char_details{
                                                let chars = &self.pak_files[i].characteristic;
                                                let (tag_text, tag_fill, tag_stroke) =
                                                    tag_colors(&chars);
                                                self.metadata_chip(
                                                    ui,
                                                    &format!("{}",&chars),
                                                    tag_text,
                                                    tag_fill,
                                                    tag_stroke,
                                                );
                                            }
                                        });
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                        let mut enabled = pak_enabled;
                                        let toggler = ui.add(ios_widget::toggle(&mut enabled));
                                        if toggler.clicked() {
                                            toggler_clicked  = true;
                                            let toggled_path = pak_path.clone();
                                            let enable_mod = enabled;
                                            if let Some(new_path) =
                                                Self::toggle_mod_file(&toggled_path, enable_mod)
                                            {
                                                self.update_tag_path(&toggled_path, &new_path);
                                                if let Err(e) = self.save_state() {
                                                    warn!(error = %e, "Failed to save tag path after toggle");
                                                }
                                                let pak_file = &mut self.pak_files[i];
                                                pak_file.path = new_path;
                                                pak_file.enabled = enabled;
                                                if self.current_pak_file_idx == Some(i) {
                                                    self.select_mod(i);
                                                }
                                            } else {
                                                self.pak_files[i].enabled = !enabled;
                                            }
                                        }
                                    });
                                });
                            });

                        let row_rect = row.response.rect;
                        let clickable_rect = egui::Rect::from_min_max(
                            row_rect.min,
                            egui::pos2(row_rect.max.x - 70.0, row_rect.max.y),
                        );

                        let row_response = ui
                            .interact(
                                clickable_rect,
                                ui.make_persistent_id(("mod_row", raw_path.as_str())),
                                egui::Sense::click(),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(format!("View files and details\n{}", raw_path));


                        if row_response.clicked() && !toggler_clicked{
                            self.select_mod(i);
                        }

                        row_response.context_menu(|ui| {
                            self.show_mod_context_menu(ui, i, &pak_path, is_iostore);
                        });
                        ui.add_space(4.0);
                    }
                });
            });
    }

    fn metadata_chip(
        &self,
        ui: &mut egui::Ui,
        text: &str,
        text_color: Color32,
        fill: Color32,
        stroke: Color32,
    ) {
        egui::Frame::NONE
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, stroke))
            .corner_radius(3)
            .inner_margin(egui::Margin::symmetric(5, 2))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(text)
                        .size((self.default_font_size - 5.0).max(10.0))
                        .color(text_color),
                );
            });
    }

    fn show_mod_context_menu(
        &mut self,
        ui: &mut egui::Ui,
        i: usize,
        pak_path: &Path,
        is_iostore: bool,
    ) {
        self.show_tag_context_menu(ui, pak_path);
        ui.separator();
        if ui.button("Extract pak to directory").clicked() {
            info!(
                mod_name = %pak_path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("<unknown>"),
                is_iostore,
                "Preparing extraction"
            );
            self.current_pak_file_idx = Some(i);
            let dir = rfd::FileDialog::new().pick_folder();
            if let Some(dir) = dir {
                let mod_name = pak_path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("extracted_mod")
                    .to_string();
                let to_create = dir.join(&mod_name);
                if let Err(e) = fs::create_dir_all(&to_create) {
                    error!(mod_name = %mod_name, error = %e, "Failed to create extraction directory");
                    return;
                }

                let mut utoc_path = pak_path.to_path_buf();
                utoc_path.set_extension("utoc");

                if utoc_path.exists() {
                    let Some(game_chunk_path) = self.game_chunk_path.clone() else {
                        warn!("Cannot convert IoStore mod to legacy without a detected game chunk path");
                        return;
                    };

                    #[cfg(all(windows, not(debug_assertions)))]
                    {
                        crate::ensure_console();
                        crate::redirect_stdio();
                    }
                    let result = to_legacy_uasset_fast(
                        pak_path.to_path_buf(),
                        dir,
                        self.game_path.clone(),
                        game_chunk_path,
                    );
                    #[cfg(all(windows, not(debug_assertions)))]
                    crate::free_console();
                    if let Err(e) = result {
                        error!(error = %e, "Failed to fast-convert IoStore mod to legacy asset");
                    }
                } else {
                    let reader = match Self::open_pak_reader(pak_path) {
                        Ok(reader) => reader,
                        Err(e) => {
                            error!(mod_name = %mod_name, error = %e, "Failed to open mod for extraction");
                            return;
                        }
                    };
                    let installable_mod = InstallableMod {
                        mod_name: mod_name.clone(),
                        mod_type: "".to_string(),
                        reader: Option::from(reader),
                        mod_path: pak_path.to_path_buf(),
                        ..Default::default()
                    };
                    if let Err(e) = extract_pak_to_dir(&installable_mod, to_create) {
                        error!(mod_name = %mod_name, error = %e, "Failed to extract mod");
                    }
                }
            }
        }
        if is_iostore && self.game_chunk_path.is_some() {
            if ui.button("To legacy asset").clicked() {
                let dir = rfd::FileDialog::new().pick_folder();
                if let Some(dir) = dir {
                    let Some(game_chunk_path) = self.game_chunk_path.clone() else {
                        warn!("Cannot convert to legacy without a detected game chunk path");
                        return;
                    };
                    #[cfg(all(windows, not(debug_assertions)))]
                    {
                        crate::ensure_console();
                        crate::redirect_stdio();
                    }

                    if let Err(e) = to_legacy_uasset(
                        pak_path.to_path_buf(),
                        dir,
                        game_chunk_path,
                        &AtomicI32::new(0),
                    ) {
                        error!(error = %e, "Failed to convert mod to legacy asset");
                    }
                    #[cfg(all(windows, not(debug_assertions)))]
                    crate::free_console();
                }
            }

            if ui.button("Fix KawaiiPhysics").clicked() {
                let Some(game_chunk_path) = self.game_chunk_path.clone() else {
                    warn!("Cannot fix KawaiiPhysics without a detected game chunk path");
                    return;
                };
                self.update_kawaii_usmap_if_needed(true);
                let Some(kawaii_physics_usmap) = self.kawaii_physics_usmap.clone() else {
                    rfd::MessageDialog::new()
                        .set_buttons(MessageButtons::Ok)
                        .set_title("Missing mapping file")
                        .set_description("Select a KawaiiPhysics unversioned mapping file before fixing KawaiiPhysics.")
                        .show();
                    return;
                };
                let mod_name = pak_path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("fixed_mod")
                    .to_string();
                let installable_mod = InstallableMod {
                    mod_name,
                    mod_type: "".to_string(),
                    repak: false,
                    fix_mesh: false,
                    kawaii_porter: true,
                    is_dir: false,
                    reader: None,
                    mod_path: pak_path.to_path_buf(),
                    mount_point: "../../../".to_string(),
                    path_hash_seed: "00000000".to_string(),
                    compression: repak::Compression::Oodle,
                    total_files: 1,
                    iostore: true,
                    is_archived: false,
                    editing: false,
                    enabled: true,
                    obfuscated: false,
                };

                #[cfg(all(windows, not(debug_assertions)))]
                {
                    crate::ensure_console();
                    crate::redirect_stdio();
                }

                let result = fix_installed_iostore_kawaii_physics(
                    &installable_mod,
                    &self.game_path,
                    Arc::new(AtomicI32::new(0)),
                    &Some(game_chunk_path),
                    &Some(kawaii_physics_usmap),
                );

                #[cfg(all(windows, not(debug_assertions)))]
                crate::free_console();

                if let Err(e) = result {
                    error!(error = %e, "Failed to fix installed IoStore KawaiiPhysics");
                }
            }
        }
        if ui.button("Delete mod").clicked() {
            if Self::delete_mod_files(pak_path).is_err() {
                return;
            }
            self.remove_all_tags_for_mod(pak_path);
            if let Err(e) = self.save_state() {
                warn!(error = %e, "Failed to save tag cleanup after delete");
            }
            self.current_pak_file_idx = None;
            self.table = None;
            self.selected_pak_details = None;
        }
    }

    fn show_tag_context_menu(&mut self, ui: &mut egui::Ui, mod_path: &Path) {
        ui.menu_button("Tags", |ui| {
            ui.horizontal(|ui| {
                ui.label("New:");
                ui.add(
                    TextEdit::singleline(&mut self.new_tag_name)
                        .hint_text("tag name")
                        .desired_width(120.0),
                );
            });
            let create_button = Button::new(RichText::new("Create").color(Color32::WHITE))
                .fill(Color32::from_rgb(186, 31, 59))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 70, 105)));
            if ui.add(create_button).clicked() {
                let tag = self.new_tag_name.trim().to_string();
                if !tag.is_empty() {
                    self.add_tag_to_mod(mod_path, &tag);
                    self.new_tag_name.clear();
                    if let Err(e) = self.save_state() {
                        warn!(error = %e, "Failed to save tag assignment");
                    }
                    ui.close_menu();
                }
            }

            let catalog = self.tag_catalog.clone();
            for tag in catalog {
                let (text_color, fill, stroke) = tag_colors(&tag);
                let tag_button = Button::new(RichText::new(format!("#{tag}")).color(text_color))
                    .fill(fill)
                    .stroke(Stroke::new(1.0_f32, stroke));
                if ui.add(tag_button).clicked() {
                    self.add_tag_to_mod(mod_path, &tag);
                    if let Err(e) = self.save_state() {
                        warn!(error = %e, "Failed to save tag assignment");
                    }
                    ui.close_menu();
                }
            }
        });

        let tags = self.tags_for_mod(mod_path);
        if !tags.is_empty() {
            ui.menu_button("Remove tag", |ui| {
                for tag in tags {
                    if ui.button(format!("Remove #{tag}")).clicked() {
                        self.remove_tag_from_mod(mod_path, &tag);
                        if let Err(e) = self.save_state() {
                            warn!(error = %e, "Failed to save tag removal");
                        }
                        ui.close_menu();
                    }
                }
            });
        }
    }

    fn category_options(&self) -> Vec<String> {
        let mut categories = self
            .pak_files
            .iter()
            .map(|entry| entry.category.clone())
            .filter(|category| category != "Pending")
            .collect::<Vec<_>>();
        categories.sort();
        categories.dedup();
        categories
    }

    fn tags_for_mod(&self, mod_path: &Path) -> Vec<String> {
        self.mod_tags
            .iter()
            .find(|assignment| same_mod_identity(&assignment.path, mod_path))
            .map(|assignment| assignment.tags.clone())
            .unwrap_or_default()
    }

    fn add_tag_to_mod(&mut self, mod_path: &Path, tag: &str) {
        let tag = tag.trim();
        if tag.is_empty() {
            return;
        }

        if !self.tag_catalog.iter().any(|existing| existing == tag) {
            self.tag_catalog.push(tag.to_string());
            self.tag_catalog.sort();
        }

        if let Some(assignment) = self
            .mod_tags
            .iter_mut()
            .find(|assignment| same_mod_identity(&assignment.path, mod_path))
        {
            if !assignment.tags.iter().any(|existing| existing == tag) {
                assignment.tags.push(tag.to_string());
                assignment.tags.sort();
            }
            return;
        }

        self.mod_tags.push(ModTagAssignment {
            path: mod_path.to_path_buf(),
            tags: vec![tag.to_string()],
        });
    }

    fn remove_tag_from_mod(&mut self, mod_path: &Path, tag: &str) {
        if let Some(assignment) = self
            .mod_tags
            .iter_mut()
            .find(|assignment| same_mod_identity(&assignment.path, mod_path))
        {
            assignment.tags.retain(|existing| existing != tag);
        }
        self.mod_tags
            .retain(|assignment| !assignment.tags.is_empty());
    }

    fn remove_all_tags_for_mod(&mut self, mod_path: &Path) {
        self.mod_tags
            .retain(|assignment| !same_mod_identity(&assignment.path, mod_path));
    }

    fn update_tag_path(&mut self, old_path: &Path, new_path: &Path) {
        if let Some(assignment) = self
            .mod_tags
            .iter_mut()
            .find(|assignment| same_mod_identity(&assignment.path, old_path))
        {
            assignment.path = new_path.to_path_buf();
        }
    }
    fn config_path() -> PathBuf {
        let mut path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("repak_manager");
        if !path.exists() {
            if let Err(e) = fs::create_dir_all(&path) {
                warn!(
                    error = %e,
                    "Failed to create config directory"
                );
            } else {
                debug!("Created config directory");
            }
        }

        path.push("repak_mod_manager.json");

        path
    }

    #[instrument(skip(ctx))]
    pub fn load(ctx: &eframe::CreationContext, path_reset: bool) -> std::io::Result<Self> {
        let (tx, rx) = channel();
        let path = Self::config_path();
        let mut persist_config = false;
        let mut shit = if path.exists() {
            info!("Loading config");
            let data = fs::read_to_string(path)?;
            let mut config: Self = serde_json::from_str(&data)?;

            if path_reset {
                info!("--path-reset requested; forcing Steam path detection");
                if let Some(path) = find_marvel_rivals() {
                    let mods_path = path.join("~mods").clean();
                    if let Err(e) = fs::create_dir_all(&mods_path) {
                        warn!(
                            error = %e,
                            "Failed to create detected mods directory"
                        );
                    } else {
                        info!(
                            detected_mods_path_exists = mods_path.exists(),
                            "Using detected Steam mods path"
                        );
                    }
                    config.game_path = mods_path;
                    persist_config = true;
                } else {
                    warn!(
                        "Steam install was not detected during --path-reset; keeping configured path"
                    );
                }
            }

            debug!("Setting custom style");
            setup_custom_style(&ctx.egui_ctx);
            debug!("Setting font size: {}", config.default_font_size);
            set_custom_font_size(&ctx.egui_ctx, config.default_font_size);

            config.set_game_pakchunk_path();
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
            config.welcome_screen = Some(ShowWelcome {});
            config.receiver = Some(rx);
            config.watcher_tx = Some(tx.clone());

            Ok(config)
        } else {
            info!("First launch creating config");
            let mut x = Self::new(ctx);
            x.welcome_screen = Some(ShowWelcome {});
            x.hide_welcome = false;
            x.receiver = Some(rx);
            x.watcher_tx = Some(tx.clone());
            Ok(x)
        };

        if let Ok(ref mut shit) = shit {
            shit.restart_game_path_watcher();
            shit.collect_pak_files();
            if persist_config {
                if let Err(e) = shit.save_state() {
                    warn!(error = %e, "Failed to persist --path-reset path update to config");
                }
            }
        }

        shit
    }
    fn save_state(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)?;
        info!("Saving config");
        fs::write(path, json)?;
        Ok(())
    }

    fn update_kawaii_usmap_if_needed(&mut self, needs_mapping: bool) {
        if !needs_mapping {
            return;
        }

        match latest_depot_usmap_path(self.kawaii_physics_usmap.as_deref()) {
            Ok(Some(path)) => {
                self.kawaii_physics_usmap = Some(path);
                if let Err(e) = self.save_state() {
                    warn!(error = %e, "Failed to save updated mapping file path");
                }
            }
            Ok(None) => {}
            Err(e) => {
                warn!(error = %e, "Failed to update KawaiiPhysics mapping file");
            }
        }
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

    #[instrument(skip(self, ctx))]
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
                    info!(
                        dropped_items = dropped_files.len(),
                        game_path_exists = self.game_path.exists(),
                        "Processing dropped items"
                    );
                    let mods = map_dropped_file_to_mods(&dropped_files);
                    if mods.is_empty() {
                        error!("No installable mods found in dropped items");
                        return;
                    }
                    self.update_kawaii_usmap_if_needed(mods_need_kawaii_mapping(&mods));
                    self.file_drop_viewport_open = true;
                    debug!(installable_mods = mods.len(), "Prepared installable mods from dropped items");
                    self.install_mod_dialog = Some(ModInstallRequest::new(
                        mods,
                        self.game_path.clone(),
                        &self.game_chunk_path,
                        &self.kawaii_physics_usmap,
                    ));

                    if let Some(dialog) = &self.install_mod_dialog {
                        trace!("Install dialog payload: {:#?}", dialog.mods);
                    }
                } else {
                    warn!("Dropped items contained unsupported files; only directories, .pak, .zip, and .rar are accepted");
                }
            }
        });
    }

    #[instrument(skip(self, ui))]
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
                        info!("Install mods cancelled before selecting files");
                        return;
                    }

                    let mods = map_paths_to_mods(&mod_files);
                    if mods.is_empty() {
                        error!("Selected files did not contain installable mods");
                        return;
                    }

                    self.update_kawaii_usmap_if_needed(mods_need_kawaii_mapping(&mods));
                    info!(selected_mods = mods.len(), "Prepared mods from file picker");
                    self.file_drop_viewport_open = true;
                    self.install_mod_dialog = Some(ModInstallRequest::new(
                        mods,
                        self.game_path.clone(),
                        &self.game_chunk_path,
                        &self.kawaii_physics_usmap,
                    ));
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
                        info!("Pack folder cancelled before selecting directories");
                        return;
                    }

                    let mods = map_paths_to_mods(&mod_files);
                    if mods.is_empty() {
                        error!("Selected directories did not contain installable mods");
                        return;
                    }
                    info!(
                        selected_mods = mods.len(),
                        "Prepared mods from directory picker"
                    );
                    self.update_kawaii_usmap_if_needed(mods_need_kawaii_mapping(&mods));
                    self.file_drop_viewport_open = true;
                    self.install_mod_dialog = Some(ModInstallRequest::new(
                        mods,
                        self.game_path.clone(),
                        &self.game_chunk_path,
                        &self.kawaii_physics_usmap,
                    ));
                }
                ui.separator();
                if ui.button("Select Mapping file").clicked() {
                    ui.close_menu();
                    if let Some(path) = FileDialog::new()
                        .set_title("Select mapping file")
                        .add_filter("Unversioned mapping", &["usmap"])
                        .pick_file()
                    {
                        self.kawaii_physics_usmap = Some(path.clean());
                        if let Err(e) = self.save_state() {
                            warn!(error = %e, "Failed to save mapping file path");
                        }
                    }
                }
                if let Some(path) = &self.kawaii_physics_usmap {
                    ui.label(format!(
                        "{}",
                        path.file_stem().unwrap_or_default().display()
                    ));
                    if ui.button("Clear Mapping file").clicked() {
                        self.kawaii_physics_usmap = None;
                        if let Err(e) = self.save_state() {
                            warn!(error = %e, "Failed to clear mapping file path");
                        }
                    }
                }
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("Settings", |ui| {
                ui.label("Font Size: ");
                ui.add(egui::Slider::new(&mut self.default_font_size, 12.0..=32.0));
                set_custom_font_size(ui.ctx(), self.default_font_size);
                ui.horizontal(|ui| {
                    ui.label("Show Load Order Suffix");
                    ui.add(ios_widget::toggle(&mut self.show_load_order_suffix));
                });
                ui.horizontal(|ui| {
                    ui.label("Show Character Details");
                    ui.add(ios_widget::toggle(&mut self.show_char_details));
                });
            });

            if ui.button("Donate").clicked() {
                self.hide_welcome = false;
            }
        });

        Ok(())
    }

    #[instrument(skip(self, ui))]
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
                        self.game_path = path.clean();
                        if let Err(e) = fs::create_dir_all(&self.game_path) {
                            warn!(error = %e, "Failed to create selected mod directory");
                        }
                        self.set_game_pakchunk_path();
                        self.launch_game_paths = None;
                        self.current_pak_file_idx = None;
                        self.table = None;
                        self.selected_pak_details = None;
                        self.collect_pak_files();
                        self.restart_game_path_watcher();
                        if let Err(e) = self.save_state() {
                            warn!(error = %e, "Failed to save selected mod path");
                        }
                    }
                }
                flex_ui.add_ui(item(), |ui| {
                    let x = ui.add_enabled(self.game_path.exists(), Button::new("Open mod folder"));
                    if x.clicked() {
                        info!(
                            mod_folder_exists = self.game_path.exists(),
                            "Opening mod folder"
                        );
                        #[cfg(target_os = "windows")]
                        {
                            if let Err(e) = crate::launch_game::shell_open_path(&self.game_path) {
                                error!(error = %e, "Failed to open folder");
                                rfd::MessageDialog::new()
                                    .set_buttons(MessageButtons::Ok)
                                    .set_title("Failed to open mod folder")
                                    .set_description(e)
                                    .show();
                            } else {
                                info!("Opened mod folder");
                            }
                        }

                        #[cfg(target_os = "linux")]
                        {
                            debug!(
                                mod_folder_exists = self.game_path.exists(),
                                "Opening mod folder"
                            );
                            let _ = std::process::Command::new("xdg-open")
                                .arg(self.game_path.to_string_lossy().to_string())
                                .spawn();
                        }
                    }
                });
                flex_ui.add_ui(item(), |ui| {
                    let launch_check = self
                        .launch_game_paths
                        .get_or_insert_with(crate::launch_game::detect_game_launch_paths);
                    let (launch_enabled, hover_text) = match &launch_check {
                        Ok(paths) => (
                            true,
                            format!(
                                "Launch Marvel Rivals via Steam ({})",
                                paths.paks_path.display()
                            ),
                        ),
                        Err(e) => (false, e.clone()),
                    };

                    let button = Button::new(RichText::new("Launch Game").color(Color32::WHITE))
                        .fill(RED_THEME_COLOR)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(255, 86, 118)));
                    let response = ui
                        .add_enabled(launch_enabled, button)
                        .on_hover_text(hover_text);

                    if response.clicked() {
                        match crate::launch_game::launch_detected_game() {
                            Ok(()) => info!("Launch game requested"),
                            Err(e) => {
                                error!(error = %e, "Failed to launch game");
                                rfd::MessageDialog::new()
                                    .set_buttons(MessageButtons::Ok)
                                    .set_title("Failed to launch game")
                                    .set_description(e)
                                    .show();
                            }
                        }
                    }
                });
            });
    }

    #[instrument(skip(pak_path), fields(has_utoc = pak_path.with_extension("utoc").exists(), has_ucas = pak_path.with_extension("ucas").exists()))]
    fn delete_mod_files(pak_path: &Path) -> std::io::Result<()> {
        let utoc_path = pak_path.with_extension("utoc");
        let ucas_path = pak_path.with_extension("ucas");
        let files_to_delete = [pak_path.to_path_buf(), utoc_path, ucas_path];

        info!("Deleting mod files");

        for file in files_to_delete {
            if !file.exists() {
                debug!(
                    file_ext = %file.extension().and_then(|ext| ext.to_str()).unwrap_or("<none>"),
                    "Skipping missing companion file"
                );
                continue;
            }

            fs::remove_file(&file)?;
            info!(
                file_ext = %file.extension().and_then(|ext| ext.to_str()).unwrap_or("<none>"),
                "Deleted companion file"
            );
        }

        Ok(())
    }

    #[instrument(skip(current_path), fields(enable_mod, source_ext = %current_path.extension().and_then(|ext| ext.to_str()).unwrap_or("<none>")))]
    fn toggle_mod_file(current_path: &PathBuf, enable_mod: bool) -> Option<PathBuf> {
        let destination_path = if enable_mod {
            current_path.with_extension("pak")
        } else {
            current_path.with_extension("bak_repak")
        };

        info!(
            destination_ext = %destination_path.extension().and_then(|ext| ext.to_str()).unwrap_or("<none>"),
            "Toggling mod"
        );

        match std::fs::rename(current_path, &destination_path) {
            Ok(_) => {
                info!(
                    destination_ext = %destination_path.extension().and_then(|ext| ext.to_str()).unwrap_or("<none>"),
                    "Toggle complete"
                );
                Some(destination_path)
            }
            Err(e) => {
                warn!(error = ?e, "Toggle failed");
                rfd::MessageDialog::new()
                    .set_buttons(MessageButtons::Ok)
                    .set_title("Failed to toggle mod")
                    .set_description("Failed to rename pak file. Make sure game is not running.")
                    .show();
                None
            }
        }
    }
}
impl eframe::App for RepakModManager {
    #[instrument(skip(self, ctx, _frame))]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(ref mut welcome) = self.welcome_screen {
            if !self.hide_welcome {
                welcome.welcome_screen(ctx, &mut self.hide_welcome);
            }
        }
        self.process_metadata_messages(ctx);

        let mut collect_pak = false;

        if !self.file_drop_viewport_open && self.install_mod_dialog.is_some() {
            self.install_mod_dialog = None;
        }

        if self.install_mod_dialog.is_none() {
            if let Some(ref receiver) = &self.receiver {
                while let Ok(event) = receiver.try_recv() {
                    match event.kind {
                        EventKind::Any => {
                            warn!("Received watcher event without a concrete kind")
                        }
                        EventKind::Other | EventKind::Access(_) => {}
                        _ => {
                            self.pending_refresh_at =
                                Some(Instant::now() + Duration::from_millis(500));
                        }
                    }
                }
            }
        }
        // if install_mod_dialog is open we dont want to listen to events

        if self
            .pending_refresh_at
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.pending_refresh_at = None;
            collect_pak = true;
        }

        if collect_pak {
            trace!("Filesystem watcher requested mod refresh");
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
            if let Err(e) = self.save_state() {
                warn!(error = %e, "Failed to save config while closing");
            }
        }
        self.check_drop(ctx);
        if let Some(ref mut install_mod) = self.install_mod_dialog {
            if self.file_drop_viewport_open {
                install_mod.new_mod_dialog(ctx, &mut self.file_drop_viewport_open);
            }
        }
    }
}

fn normalize_mod_display_name(name: &str) -> String {
    name.replace("_9999999_P", "").replace("_999999_P", "")
}

fn classify_mod_metadata(job: &MetadataJob) -> Result<MetadataResult, String> {
    let files = files_for_metadata(&job.path, job.is_iostore)?;
    let file_count = Some(files.len());
    Ok(MetadataResult {
        generation: job.generation,
        identity: job.identity.clone(),
        signature: job.signature,
        category: detect_mod_category(&files),
        characteristic: get_current_pak_characteristics(files),
        obfuscated: job.obfuscated.clone(),
        file_count,
    })
}

fn files_for_metadata(pak_path: &Path, is_iostore: bool) -> Result<Vec<String>, String> {
    if is_iostore {
        return read_utoc_package_names(&pak_path.with_extension("utoc"));
    }

    let mut builder = repak::PakBuilder::new();
    builder = builder.key(AES_KEY.clone().0);
    let file = File::open(pak_path).map_err(|e| format!("Failed to open mod: {e}"))?;
    let pak = builder
        .reader(&mut BufReader::new(file))
        .map_err(|e| format!("Failed to read mod: {e}"))?;
    Ok(pak.files())
}

fn mod_file_state(path: &Path) -> Option<(bool, bool)> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("pak") => Some((true, true)),
        Some("pak_disabled") | Some("bak_repak") => Some((false, true)),
        _ => None,
    }
}

fn mod_file_signature(path: &Path) -> Option<ModFileSignature> {
    let mut size = 0u64;
    let mut modified_secs = 0u64;
    let companions = [
        path.to_path_buf(),
        path.with_extension("utoc"),
        path.with_extension("ucas"),
    ];

    for companion in companions {
        if !companion.exists() {
            continue;
        }
        let metadata = fs::metadata(&companion).ok()?;
        size = size.saturating_add(metadata.len());
        let modified = metadata
            .modified()
            .ok()
            .and_then(system_time_to_secs)
            .unwrap_or_default();
        modified_secs = modified_secs.max(modified);
    }

    Some(ModFileSignature {
        size,
        modified_secs,
    })
}

fn system_time_to_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

fn has_mod_suffix(name: &str) -> bool {
    name.ends_with("_9999999_P") || name.ends_with("_999999_P")
}

fn detect_mod_category(files: &[String]) -> String {
    if files.iter().any(|file| {
        let name = Path::new(file)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        name.starts_with("sk_") || name.starts_with("sm_")
    }) {
        return "Mesh".to_string();
    }

    if files.iter().any(|file| file.contains("WwiseAudio")) {
        return "Audio".to_string();
    }

    if files.iter().any(|file| {
        file.strip_prefix("Marvel/Content/Marvel/")
            .or_else(|| file.strip_prefix("/Game/Marvel/"))
            .unwrap_or(file)
            .starts_with("UI/")
    }) {
        return "UI".to_string();
    }

    if files.iter().any(|file| {
        file.strip_prefix("Marvel/Content/Marvel/")
            .or_else(|| file.strip_prefix("/Game/Marvel/"))
            .unwrap_or(file)
            .starts_with("Movies/")
    }) {
        return "Movies".to_string();
    }

    if files.iter().any(|file| {
        file.strip_prefix("Marvel/Content/Marvel/")
            .or_else(|| file.strip_prefix("/Game/Marvel/"))
            .unwrap_or(file)
            .starts_with("Characters/")
    }) {
        return "Texture".to_string();
    }

    "Other".to_string()
}

fn filter_label(prefix: &str, selected: &[String]) -> String {
    match selected.len() {
        0 => format!("{prefix}: All"),
        1 => selected[0].clone(),
        count => format!("{prefix}: {count}"),
    }
}

fn toggle_filter_value(filters: &mut Vec<String>, value: String, selected: bool) {
    if selected {
        if !filters.contains(&value) {
            filters.push(value);
            filters.sort();
        }
    } else {
        filters.retain(|existing| existing != &value);
    }
}

fn category_colors(category: &str) -> (Color32, Color32, Color32) {
    match category {
        "Mesh" => rgb3((255, 213, 137), (80, 55, 28), (142, 96, 44)),
        "Texture" => rgb3((147, 231, 207), (24, 67, 61), (45, 126, 113)),
        "Audio" => rgb3((213, 181, 255), (60, 42, 90), (111, 78, 164)),
        "UI" => rgb3((160, 203, 255), (31, 56, 88), (54, 105, 168)),
        "Movies" => rgb3((255, 171, 198), (82, 37, 52), (151, 68, 94)),
        _ => rgb3((210, 210, 210), (54, 54, 54), (86, 86, 86)),
    }
}

fn tag_colors(tag: &str) -> (Color32, Color32, Color32) {
    const PALETTE: [(Color32, Color32, Color32); 20] = [
        rgb3((149, 214, 255), (25, 57, 78), (51, 113, 154)),
        rgb3((170, 236, 182), (31, 67, 40), (59, 127, 75)),
        rgb3((255, 207, 145), (77, 54, 26), (143, 99, 44)),
        rgb3((230, 180, 255), (65, 42, 79), (122, 78, 150)),
        rgb3((255, 174, 174), (78, 38, 38), (148, 70, 70)),
        rgb3((147, 231, 207), (24, 67, 61), (45, 126, 113)),
        rgb3((255, 181, 221), (78, 38, 61), (148, 70, 116)),
        rgb3((225, 226, 149), (66, 67, 31), (125, 127, 58)),
        rgb3((191, 219, 254), (30, 58, 138), (59, 130, 246)),
        rgb3((196, 181, 253), (76, 29, 149), (139, 92, 246)),
        rgb3((253, 186, 116), (124, 45, 18), (249, 115, 22)),
        rgb3((252, 165, 165), (127, 29, 29), (239, 68, 68)),
        rgb3((134, 239, 172), (20, 83, 45), (34, 197, 94)),
        rgb3((103, 232, 249), (22, 78, 99), (6, 182, 212)),
        rgb3((216, 180, 254), (88, 28, 135), (168, 85, 247)),
        rgb3((253, 224, 71), (113, 63, 18), (234, 179, 8)),
        rgb3((244, 114, 182), (131, 24, 67), (219, 39, 119)),
        rgb3((163, 230, 53), (63, 98, 18), (132, 204, 22)),
        rgb3((125, 211, 252), (12, 74, 110), (14, 165, 233)),
        rgb3((251, 191, 36), (120, 53, 15), (245, 158, 11)),
    ];

    let hash = fnv1a(tag.as_bytes());
    PALETTE[hash as usize % PALETTE.len()]
}

const fn rgb3(
    bg: (u8, u8, u8),
    text: (u8, u8, u8),
    stroke: (u8, u8, u8),
) -> (Color32, Color32, Color32) {
    (
        Color32::from_rgb(bg.0, bg.1, bg.2),
        Color32::from_rgb(text.0, text.1, text.2),
        Color32::from_rgb(stroke.0, stroke.1, stroke.2),
    )
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

fn same_mod_identity(left: &Path, right: &Path) -> bool {
    normalized_mod_identity(left) == normalized_mod_identity(right)
}

fn normalized_mod_identity(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".bak_repak")
        .or_else(|| file_name.strip_suffix(".pak_disabled"))
        .or_else(|| file_name.strip_suffix(".pak"))
        .unwrap_or(file_name);

    parent.join(stem)
}

fn normalized_mod_identity_string(path: &Path) -> String {
    normalized_mod_identity(path)
        .to_string_lossy()
        .to_ascii_lowercase()
}
