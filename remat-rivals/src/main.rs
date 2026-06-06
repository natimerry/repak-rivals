#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{
    self, Align, Button, Color32, ComboBox, DragValue, RichText, ScrollArea, Sense, Stroke,
    TextEdit, Theme, Vec2,
};
use egui_plot::{Line, Plot, PlotPoints};
use rfd::FileDialog;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use tracing_subscriber::filter::LevelFilter;
use walkdir::WalkDir;

const RIVALS_AES_KEY: &str = "0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74";
const DEFAULT_HIDDEN_MATERIAL_BITMAPS: [u64; 3] = [0x0FFF0000, 0x0FFF0000, 0x0EFB0000];
const DEFAULT_MASK_SLOTS: usize = 32;
const MAX_MASK_SLOTS: usize = 64;

fn main() -> eframe::Result {
    init_tracing();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 780.0])
            .with_min_inner_size([1120.0, 660.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Remat Rivals",
        options,
        Box::new(|cc| {
            setup_style(&cc.egui_ctx);
            Ok(Box::new(RematApp::default()))
        }),
    )
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .with_target(false)
        .try_init();
}

fn setup_style(ctx: &egui::Context) {
    ctx.set_theme(Theme::Dark);
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(14, 19, 18);
    visuals.window_fill = Color32::from_rgb(18, 25, 24);
    visuals.extreme_bg_color = Color32::from_rgb(8, 10, 10);
    visuals.faint_bg_color = Color32::from_rgb(24, 34, 32);
    visuals.hyperlink_color = Color32::from_rgb(80, 220, 205);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(47, 172, 157, 92);
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(245, 182, 92));
    visuals.widgets.active.bg_fill = Color32::from_rgb(39, 120, 111);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(34, 82, 77);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(24, 34, 32);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(17, 24, 23);

    ctx.set_visuals_of(Theme::Dark, visuals.clone());
    ctx.set_visuals_of(Theme::Light, visuals);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tab {
    Setup,
    HiddenMaterials,
    Kawaii,
    Curves,
    Preview,
    Run,
}

impl Tab {
    const ALL: [Self; 6] = [
        Self::Setup,
        Self::HiddenMaterials,
        Self::Kawaii,
        Self::Curves,
        Self::Preview,
        Self::Run,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Setup => "Setup",
            Self::HiddenMaterials => "Hidden Mats",
            Self::Kawaii => "Kawaii Values",
            Self::Curves => "Curves",
            Self::Preview => "Preview",
            Self::Run => "Run",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HiddenMaterialMode {
    Auto,
    Default,
    Custom,
}

impl HiddenMaterialMode {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Default => "Default",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Clone, Debug)]
struct CurveState {
    key: &'static str,
    label: &'static str,
    points: Vec<CurvePoint>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct CurvePoint {
    time: f32,
    value: f32,
}

#[derive(Clone, Debug)]
struct KawaiiValueState {
    force_rebuild: bool,
    patch_kawaii: bool,
    apply_scalars: bool,
    apply_startup: bool,
    apply_gravity: bool,
    use_curves: bool,
    clear_curve_data: bool,
    clear_external_forces: bool,
    disable_wind: bool,
    world_damping_location: f32,
    world_damping_rotation: f32,
    stiffness: f32,
    damping: f32,
    gravity_scale: f32,
    teleport_distance_threshold: f32,
    teleport_rotation_threshold: f32,
    enable_warm_up: bool,
    warm_up_frames: i32,
    use_world_space_gravity: bool,
    use_project_gravity: bool,
    gravity_vector: [f32; 3],
    simulation_space: String,
}

impl Default for KawaiiValueState {
    fn default() -> Self {
        Self {
            force_rebuild: true,
            patch_kawaii: true,
            apply_scalars: false,
            apply_startup: false,
            apply_gravity: false,
            use_curves: true,
            clear_curve_data: false,
            clear_external_forces: false,
            disable_wind: false,
            world_damping_location: 0.15,
            world_damping_rotation: 0.15,
            stiffness: 0.55,
            damping: 0.18,
            gravity_scale: 1.0,
            teleport_distance_threshold: 300.0,
            teleport_rotation_threshold: 45.0,
            enable_warm_up: true,
            warm_up_frames: 8,
            use_world_space_gravity: false,
            use_project_gravity: true,
            gravity_vector: [0.0, 0.0, -980.0],
            simulation_space: "ComponentSpace".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PreviewParticle {
    pos: egui::Pos2,
    prev: egui::Pos2,
}

#[derive(Debug)]
struct PreviewState {
    particles: Vec<PreviewParticle>,
    animate: bool,
    wind_phase: f32,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            animate: true,
            wind_phase: 0.0,
        }
    }
}

#[derive(Debug)]
enum JobMessage {
    Log(String),
    Done(Result<String, String>),
}

struct RematApp {
    tab: Tab,
    raw_asset_dir: String,
    package_path: String,
    extract_output_dir: String,
    usmap_path: String,
    patch_hidden_materials: bool,
    hidden_mode: HiddenMaterialMode,
    hidden_bitmaps: Vec<u64>,
    hidden_slots: usize,
    values: KawaiiValueState,
    apply_curve_overrides: bool,
    curves: Vec<CurveState>,
    preview: PreviewState,
    logs: Vec<String>,
    worker: Option<Receiver<JobMessage>>,
}

impl Default for RematApp {
    fn default() -> Self {
        Self {
            tab: Tab::Setup,
            raw_asset_dir: String::new(),
            package_path: String::new(),
            extract_output_dir: String::new(),
            usmap_path: String::new(),
            patch_hidden_materials: true,
            hidden_mode: HiddenMaterialMode::Auto,
            hidden_bitmaps: DEFAULT_HIDDEN_MATERIAL_BITMAPS.to_vec(),
            hidden_slots: DEFAULT_MASK_SLOTS,
            values: KawaiiValueState::default(),
            apply_curve_overrides: false,
            curves: default_curves(),
            preview: PreviewState::default(),
            logs: Vec::new(),
            worker: None,
        }
    }
}

impl eframe::App for RematApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker();

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Remat Rivals").color(Color32::from_rgb(96, 224, 208)));
                ui.separator();
                ui.label("Material masks, KawaiiPhysics values, curves, and previewing");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if self.worker.is_some() {
                        ui.colored_label(Color32::from_rgb(245, 182, 92), "Working");
                    }
                });
            });
        });

        egui::SidePanel::left("tabs")
            .resizable(false)
            .exact_width(170.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                for tab in Tab::ALL {
                    let selected = self.tab == tab;
                    if ui
                        .add_sized([148.0, 34.0], Button::new(tab.label()).selected(selected))
                        .clicked()
                    {
                        self.tab = tab;
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Setup => self.show_setup(ui),
            Tab::HiddenMaterials => self.show_hidden_materials(ui),
            Tab::Kawaii => self.show_kawaii_values(ui),
            Tab::Curves => self.show_curves(ui),
            Tab::Preview => self.show_preview(ui, ctx),
            Tab::Run => self.show_run(ui),
        });
    }
}

impl RematApp {
    fn show_setup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Asset Inputs");
        ui.add_space(6.0);
        ui.label("Use a raw extracted asset directory for patching. Package extraction is available here and writes a raw directory first.");
        ui.add_space(12.0);

        path_picker_row(ui, "Raw asset dir", &mut self.raw_asset_dir, true);
        path_picker_row(ui, "USMAP", &mut self.usmap_path, false);

        ui.separator();
        ui.heading("Package Extraction");
        ui.label("Extraction reads the selected IoStore .utoc and writes a raw asset directory. Point the raw asset dir at that output before patching.");
        path_picker_row(ui, "Package .utoc", &mut self.package_path, false);
        path_picker_row(ui, "Extract output", &mut self.extract_output_dir, true);

        ui.horizontal(|ui| {
            let enabled = self.worker.is_none();
            if ui
                .add_enabled(enabled, Button::new("Extract Package"))
                .clicked()
            {
                self.start_extract_package();
            }
            if ui.button("Use Extract Output As Raw Dir").clicked() {
                self.raw_asset_dir = self.extract_output_dir.clone();
            }
        });
    }

    fn show_hidden_materials(&mut self, ui: &mut egui::Ui) {
        ui.heading("DefaultHiddenMaterials");
        ui.checkbox(
            &mut self.patch_hidden_materials,
            "Patch DefaultHiddenMaterials",
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Mode");
            ComboBox::from_id_salt("hidden_mode")
                .selected_text(self.hidden_mode.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.hidden_mode, HiddenMaterialMode::Auto, "Auto");
                    ui.selectable_value(
                        &mut self.hidden_mode,
                        HiddenMaterialMode::Default,
                        "Default",
                    );
                    ui.selectable_value(
                        &mut self.hidden_mode,
                        HiddenMaterialMode::Custom,
                        "Custom",
                    );
                });
        });

        match self.hidden_mode {
            HiddenMaterialMode::Auto => {
                ui.label("Auto reads LODHiddenMaterials carrier data from the asset and injects DefaultHiddenMaterials into LODInfo.");
            }
            HiddenMaterialMode::Default => {
                self.hidden_bitmaps = DEFAULT_HIDDEN_MATERIAL_BITMAPS.to_vec();
                ui.label(format!(
                    "Default mask: {}",
                    format_hidden_bitmaps(&self.hidden_bitmaps)
                ));
            }
            HiddenMaterialMode::Custom => {
                self.show_bitmap_creator(ui);
            }
        }

        ui.separator();
        ui.label("Checked or bit=1 writes true, which means the material slot is hidden by default. Unchecked or bit=0 writes false, which means visible by default.");
    }

    fn show_bitmap_creator(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Slots");
            ui.add(DragValue::new(&mut self.hidden_slots).range(1..=MAX_MASK_SLOTS));
            if ui.button("Default").clicked() {
                self.hidden_bitmaps = DEFAULT_HIDDEN_MATERIAL_BITMAPS.to_vec();
                self.hidden_slots = suggested_slot_count(&self.hidden_bitmaps);
            }
            if ui.button("Clear").clicked() {
                for bitmap in &mut self.hidden_bitmaps {
                    *bitmap = 0;
                }
            }
            if ui.button("Add LOD").clicked() {
                self.hidden_bitmaps.push(0);
            }
            if ui
                .add_enabled(self.hidden_bitmaps.len() > 1, Button::new("Remove LOD"))
                .clicked()
            {
                self.hidden_bitmaps.pop();
            }
        });

        ui.add_space(8.0);
        ScrollArea::vertical().max_height(410.0).show(ui, |ui| {
            for (lod_idx, bitmap) in self.hidden_bitmaps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.strong(format!("LOD {lod_idx}"));
                    ui.monospace(format!("0x{bitmap:016X}"));
                });

                for row_start in (0..self.hidden_slots).step_by(8) {
                    ui.horizontal(|ui| {
                        for slot in row_start..(row_start + 8).min(self.hidden_slots) {
                            let bit = 1_u64 << slot;
                            let mut checked = (*bitmap & bit) != 0;
                            if ui.checkbox(&mut checked, slot.to_string()).changed() {
                                if checked {
                                    *bitmap |= bit;
                                } else {
                                    *bitmap &= !bit;
                                }
                            }
                        }
                    });
                }
                ui.separator();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Mask");
            ui.monospace(format_hidden_bitmaps(&self.hidden_bitmaps));
        });
    }

    fn show_kawaii_values(&mut self, ui: &mut egui::Ui) {
        ui.heading("KawaiiPhysics Values");
        ui.checkbox(&mut self.values.patch_kawaii, "Patch KawaiiPhysics");
        ui.checkbox(
            &mut self.values.force_rebuild,
            "Force rebuild Chains[0] from legacy node data",
        );
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.values.apply_scalars,
            "Apply scalar physics values",
        );
        ui.add_enabled_ui(self.values.apply_scalars, |ui| {
            ui.add(SliderRow::new(
                "Stiffness",
                &mut self.values.stiffness,
                0.0..=1.0,
            ));
            ui.add(SliderRow::new(
                "Damping",
                &mut self.values.damping,
                0.0..=1.0,
            ));
            ui.add(SliderRow::new(
                "Gravity scale",
                &mut self.values.gravity_scale,
                0.0..=4.0,
            ));
            ui.add(SliderRow::new(
                "World damping location",
                &mut self.values.world_damping_location,
                0.0..=2.0,
            ));
            ui.add(SliderRow::new(
                "World damping rotation",
                &mut self.values.world_damping_rotation,
                0.0..=2.0,
            ));
        });

        ui.separator();
        ui.checkbox(
            &mut self.values.apply_startup,
            "Apply startup and reset values",
        );
        ui.add_enabled_ui(self.values.apply_startup, |ui| {
            ui.horizontal(|ui| {
                ui.label("Simulation space");
                ComboBox::from_id_salt("simulation_space")
                    .selected_text(&self.values.simulation_space)
                    .show_ui(ui, |ui| {
                        for value in ["ComponentSpace", "WorldSpace", "BoneSpace", "BaseBoneSpace"]
                        {
                            ui.selectable_value(
                                &mut self.values.simulation_space,
                                value.to_string(),
                                value,
                            );
                        }
                    });
            });
            ui.add(SliderRow::new(
                "Teleport distance",
                &mut self.values.teleport_distance_threshold,
                0.0..=2000.0,
            ));
            ui.add(SliderRow::new(
                "Teleport rotation",
                &mut self.values.teleport_rotation_threshold,
                0.0..=180.0,
            ));
            ui.checkbox(&mut self.values.enable_warm_up, "Enable warm up");
            ui.horizontal(|ui| {
                ui.label("Warm up frames");
                ui.add(DragValue::new(&mut self.values.warm_up_frames).range(0..=120));
            });
        });

        ui.separator();
        ui.checkbox(
            &mut self.values.apply_gravity,
            "Apply gravity and external force values",
        );
        ui.add_enabled_ui(self.values.apply_gravity, |ui| {
            ui.checkbox(
                &mut self.values.use_world_space_gravity,
                "Use world space gravity",
            );
            ui.checkbox(
                &mut self.values.use_project_gravity,
                "Use project gravity setting",
            );
            ui.checkbox(&mut self.values.disable_wind, "Disable wind");
            ui.checkbox(
                &mut self.values.clear_external_forces,
                "Clear external force arrays",
            );
            ui.horizontal(|ui| {
                ui.label("Gravity vector");
                for (idx, label) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(*label);
                    ui.add(DragValue::new(&mut self.values.gravity_vector[idx]).speed(5.0));
                }
            });
        });
    }

    fn show_curves(&mut self, ui: &mut egui::Ui) {
        ui.heading("Curve Overrides");
        ui.checkbox(&mut self.apply_curve_overrides, "Write curve key overrides");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.values.use_curves, "Set bUseCurve");
            ui.checkbox(
                &mut self.values.clear_curve_data,
                "Clear existing curves first",
            );
        });
        ui.label("Curve time is normalized 0..1 along the chain. The generated keys use linear interpolation.");

        ui.add_enabled_ui(self.apply_curve_overrides, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for curve in &mut self.curves {
                    curve_editor(ui, curve);
                    ui.separator();
                }
            });
        });
    }

    fn show_preview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Physics Preview");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.preview.animate, "Animate");
            if ui.button("Reset").clicked() {
                self.preview.particles.clear();
            }
        });
        ui.label("Preview is a lightweight chain approximation for tuning values and curves. It is not a byte-for-byte Unreal KawaiiPhysics simulation.");

        let desired_size = Vec2::new(ui.available_width(), 460.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(9, 13, 13));
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0_f32, Color32::from_rgb(51, 82, 78)),
            egui::StrokeKind::Inside,
        );

        if self.preview.particles.is_empty() {
            self.reset_preview(rect);
        }
        if self.preview.animate {
            self.step_preview(rect);
            ctx.request_repaint();
        }
        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some(root) = self.preview.particles.first_mut() {
                    root.pos = pointer;
                    root.prev = pointer;
                }
            }
        }

        let particles = &self.preview.particles;
        for pair in particles.windows(2) {
            painter.line_segment(
                [pair[0].pos, pair[1].pos],
                Stroke::new(3.0_f32, Color32::from_rgb(96, 224, 208)),
            );
        }
        for (idx, particle) in particles.iter().enumerate() {
            let color = if idx == 0 {
                Color32::from_rgb(245, 182, 92)
            } else {
                Color32::from_rgb(220, 234, 231)
            };
            painter.circle_filled(particle.pos, 5.0, color);
        }
    }

    fn show_run(&mut self, ui: &mut egui::Ui) {
        ui.heading("Run");
        ui.horizontal(|ui| {
            let enabled = self.worker.is_none();
            if ui
                .add_enabled(enabled, Button::new("Patch Raw Directory"))
                .clicked()
            {
                self.start_patch_directory();
            }
            if ui.button("Clear Log").clicked() {
                self.logs.clear();
            }
        });

        ui.separator();
        ui.label(format!(
            "Raw directory: {}",
            if self.raw_asset_dir.is_empty() {
                "(not set)"
            } else {
                &self.raw_asset_dir
            }
        ));
        ui.label(format!(
            "Actions: KawaiiPhysics={}, DefaultHiddenMaterials={}",
            self.values.patch_kawaii, self.patch_hidden_materials
        ));
        ui.label(format!(
            "UAsset count: {}",
            count_uassets(&self.raw_asset_dir)
        ));

        ui.separator();
        ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            for line in &self.logs {
                ui.monospace(line);
            }
        });
    }

    fn reset_preview(&mut self, rect: egui::Rect) {
        let count = 12;
        let spacing = 28.0;
        let root = egui::pos2(rect.center().x, rect.top() + 52.0);
        self.preview.particles = (0..count)
            .map(|idx| {
                let pos = egui::pos2(root.x, root.y + idx as f32 * spacing);
                PreviewParticle { pos, prev: pos }
            })
            .collect();
    }

    fn step_preview(&mut self, rect: egui::Rect) {
        if self.preview.particles.len() < 2 {
            self.reset_preview(rect);
        }

        let dt = 1.0 / 60.0;
        let damping = (1.0 - self.values.damping * 0.10).clamp(0.75, 0.999);
        let gravity = self.values.gravity_scale * 620.0;
        let stiffness_curve = self.curve_value("stiffness", 0.5);
        let stiffness = if self.apply_curve_overrides {
            stiffness_curve
        } else {
            self.values.stiffness
        }
        .clamp(0.05, 1.0);
        let wind = if self.values.disable_wind {
            0.0
        } else {
            self.preview.wind_phase.sin() * 42.0
        };
        self.preview.wind_phase += dt * 2.0;

        let root = self.preview.particles[0].pos;
        for particle in self.preview.particles.iter_mut().skip(1) {
            let velocity = (particle.pos - particle.prev) * damping;
            particle.prev = particle.pos;
            particle.pos += velocity;
            particle.pos.x += wind * dt;
            particle.pos.y += gravity * dt * dt;
        }
        self.preview.particles[0].pos = root;
        self.preview.particles[0].prev = root;

        let target_length = 28.0;
        let iterations = (3.0 + stiffness * 10.0) as usize;
        for _ in 0..iterations {
            self.preview.particles[0].pos = root;
            for idx in 1..self.preview.particles.len() {
                let prev = self.preview.particles[idx - 1].pos;
                let current = self.preview.particles[idx].pos;
                let delta = current - prev;
                let length = delta.length().max(0.001);
                let correction = delta * ((length - target_length) / length) * stiffness;
                if idx > 1 {
                    self.preview.particles[idx - 1].pos += correction * 0.5;
                }
                self.preview.particles[idx].pos -= correction;
            }
        }

        for particle in &mut self.preview.particles {
            particle.pos.x = particle
                .pos
                .x
                .clamp(rect.left() + 10.0, rect.right() - 10.0);
            particle.pos.y = particle
                .pos
                .y
                .clamp(rect.top() + 10.0, rect.bottom() - 10.0);
        }
    }

    fn curve_value(&self, key: &str, time: f32) -> f32 {
        let Some(curve) = self.curves.iter().find(|curve| curve.key == key) else {
            return 0.0;
        };
        sample_curve(&curve.points, time)
    }

    fn start_extract_package(&mut self) {
        let package = PathBuf::from(self.package_path.trim());
        let output = PathBuf::from(self.extract_output_dir.trim());
        if package.as_os_str().is_empty() || output.as_os_str().is_empty() {
            self.logs
                .push("Set package .utoc and extract output first.".to_string());
            return;
        }

        self.logs.push(format!(
            "Extraction will read {} and write raw assets to {}",
            package.display(),
            output.display()
        ));
        let (tx, rx) = mpsc::channel();
        self.worker = Some(rx);
        thread::spawn(move || {
            let result = extract_package(&package, &output).map(|_| {
                format!(
                    "Extraction complete. Set raw asset dir to {} before patching.",
                    output.display()
                )
            });
            let _ = tx.send(JobMessage::Done(result.map_err(|err| format!("{err:#}"))));
        });
    }

    fn start_patch_directory(&mut self) {
        let input = PathBuf::from(self.raw_asset_dir.trim());
        let usmap = PathBuf::from(self.usmap_path.trim());
        if input.as_os_str().is_empty() || usmap.as_os_str().is_empty() {
            self.logs
                .push("Set raw asset dir and USMAP first.".to_string());
            return;
        }
        if !self.values.patch_kawaii && !self.patch_hidden_materials {
            self.logs
                .push("Enable KawaiiPhysics, DefaultHiddenMaterials, or both.".to_string());
            return;
        }

        let Ok((patch_hidden_auto, bitmaps)) = self.hidden_request() else {
            self.logs
                .push("Custom hidden-material mask is empty.".to_string());
            return;
        };
        let edit_json = match self.edit_options_json() {
            Ok(value) => value,
            Err(error) => {
                self.logs.push(error);
                return;
            }
        };
        let edit_json = (!edit_json.is_empty()).then_some(edit_json);
        let values = self.values.clone();
        let patch_hidden_materials = self.patch_hidden_materials;

        let (tx, rx) = mpsc::channel();
        self.worker = Some(rx);
        self.logs
            .push(format!("Patching raw assets in {}", input.display()));

        thread::spawn(move || {
            let _ = tx.send(JobMessage::Log(format!(
                "Scanning {} uasset files",
                count_uassets_path(&input)
            )));
            let result = retoc::patch_uasset_directory(
                &input,
                &usmap,
                values.force_rebuild,
                values.patch_kawaii,
                patch_hidden_materials && patch_hidden_auto,
                bitmaps.as_deref(),
                edit_json.as_deref(),
            )
            .map(|ported| format!("Patch complete. Ported {ported} KawaiiPhysics anim node(s)."));
            let _ = tx.send(JobMessage::Done(result.map_err(|err| format!("{err:#}"))));
        });
    }

    fn hidden_request(&self) -> Result<(bool, Option<Vec<u64>>), ()> {
        if !self.patch_hidden_materials {
            return Ok((false, None));
        }

        match self.hidden_mode {
            HiddenMaterialMode::Auto => Ok((true, None)),
            HiddenMaterialMode::Default => {
                Ok((false, Some(DEFAULT_HIDDEN_MATERIAL_BITMAPS.to_vec())))
            }
            HiddenMaterialMode::Custom => {
                if self.hidden_bitmaps.is_empty() {
                    Err(())
                } else {
                    Ok((false, Some(self.hidden_bitmaps.clone())))
                }
            }
        }
    }

    fn edit_options_json(&self) -> Result<String, String> {
        let mut root = Map::new();

        root.insert("use_curves".to_string(), json!(self.values.use_curves));
        if self.values.clear_curve_data {
            root.insert("clear_curve_data".to_string(), json!(true));
        }

        if self.values.apply_scalars {
            root.insert("stiffness".to_string(), json!(self.values.stiffness));
            root.insert("damping".to_string(), json!(self.values.damping));
            root.insert(
                "gravity_scale".to_string(),
                json!(self.values.gravity_scale),
            );
            root.insert(
                "world_damping_location".to_string(),
                json!(self.values.world_damping_location),
            );
            root.insert(
                "world_damping_rotation".to_string(),
                json!(self.values.world_damping_rotation),
            );
        }

        if self.values.apply_startup {
            root.insert(
                "simulation_space".to_string(),
                json!(self.values.simulation_space),
            );
            root.insert(
                "teleport_distance_threshold".to_string(),
                json!(self.values.teleport_distance_threshold),
            );
            root.insert(
                "teleport_rotation_threshold".to_string(),
                json!(self.values.teleport_rotation_threshold),
            );
            root.insert(
                "enable_warm_up".to_string(),
                json!(self.values.enable_warm_up),
            );
            root.insert(
                "warm_up_frames".to_string(),
                json!(self.values.warm_up_frames),
            );
        }

        if self.values.apply_gravity {
            root.insert(
                "use_world_space_gravity".to_string(),
                json!(self.values.use_world_space_gravity),
            );
            root.insert(
                "use_project_gravity".to_string(),
                json!(self.values.use_project_gravity),
            );
            root.insert("disable_wind".to_string(), json!(self.values.disable_wind));
            root.insert(
                "clear_external_forces".to_string(),
                json!(self.values.clear_external_forces),
            );
            root.insert(
                "gravity_vector".to_string(),
                json!({
                    "x": self.values.gravity_vector[0],
                    "y": self.values.gravity_vector[1],
                    "z": self.values.gravity_vector[2],
                }),
            );
        }

        if self.apply_curve_overrides {
            let mut curves = Map::new();
            for curve in &self.curves {
                let points = curve
                    .points
                    .iter()
                    .map(|point| json!({"time": point.time, "value": point.value}))
                    .collect::<Vec<_>>();
                curves.insert(curve.key.to_string(), Value::Array(points));
            }
            root.insert("curves".to_string(), Value::Object(curves));
        }

        if root.is_empty() {
            return Ok(String::new());
        }

        serde_json::to_string(&Value::Object(root))
            .map_err(|error| format!("Failed to serialize Kawaii options: {error}"))
    }

    fn poll_worker(&mut self) {
        let mut done = false;
        if let Some(rx) = &self.worker {
            while let Ok(message) = rx.try_recv() {
                match message {
                    JobMessage::Log(line) => self.logs.push(line),
                    JobMessage::Done(Ok(line)) => {
                        self.logs.push(line);
                        done = true;
                    }
                    JobMessage::Done(Err(error)) => {
                        self.logs.push(format!("ERROR: {error}"));
                        done = true;
                    }
                }
            }
        }
        if done {
            self.worker = None;
        }
    }
}

struct SliderRow<'a> {
    label: &'a str,
    value: &'a mut f32,
    range: std::ops::RangeInclusive<f32>,
}

impl<'a> SliderRow<'a> {
    fn new(label: &'a str, value: &'a mut f32, range: std::ops::RangeInclusive<f32>) -> Self {
        Self {
            label,
            value,
            range,
        }
    }
}

impl egui::Widget for SliderRow<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            ui.label(self.label);
            ui.add(egui::Slider::new(self.value, self.range).show_value(true));
        })
        .response
    }
}

fn path_picker_row(ui: &mut egui::Ui, label: &str, value: &mut String, directory: bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add_sized(
            [ui.available_width() - 92.0, 24.0],
            TextEdit::singleline(value),
        );
        if ui.button("Browse").clicked() {
            let picked = if directory {
                FileDialog::new().pick_folder()
            } else {
                FileDialog::new().pick_file()
            };
            if let Some(path) = picked {
                *value = path.display().to_string();
            }
        }
    });
}

fn curve_editor(ui: &mut egui::Ui, curve: &mut CurveState) {
    ui.horizontal(|ui| {
        ui.strong(curve.label);
        if ui.button("Add Point").clicked() {
            curve.points.push(CurvePoint {
                time: 1.0,
                value: curve.points.last().map(|point| point.value).unwrap_or(0.5),
            });
        }
        if ui
            .add_enabled(curve.points.len() > 2, Button::new("Remove Last"))
            .clicked()
        {
            curve.points.pop();
        }
        if ui.button("Normalize Order").clicked() {
            curve
                .points
                .sort_by(|left, right| left.time.total_cmp(&right.time));
        }
    });

    let points = curve
        .points
        .iter()
        .map(|point| [point.time as f64, point.value as f64])
        .collect::<Vec<_>>();
    Plot::new(format!("curve_plot_{}", curve.key))
        .height(150.0)
        .include_x(0.0)
        .include_x(1.0)
        .include_y(0.0)
        .include_y(1.0)
        .show(ui, |plot_ui| {
            plot_ui
                .line(Line::new(PlotPoints::from(points)).color(Color32::from_rgb(96, 224, 208)));
        });

    for (idx, point) in curve.points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("Key {idx}"));
            ui.label("Time");
            ui.add(DragValue::new(&mut point.time).speed(0.01).range(0.0..=1.0));
            ui.label("Value");
            ui.add(DragValue::new(&mut point.value).speed(0.01));
        });
    }
}

fn default_curves() -> Vec<CurveState> {
    vec![
        CurveState {
            key: "stiffness",
            label: "Stiffness",
            points: vec![
                CurvePoint {
                    time: 0.0,
                    value: 0.75,
                },
                CurvePoint {
                    time: 1.0,
                    value: 0.35,
                },
            ],
        },
        CurveState {
            key: "damping",
            label: "Damping",
            points: vec![
                CurvePoint {
                    time: 0.0,
                    value: 0.2,
                },
                CurvePoint {
                    time: 1.0,
                    value: 0.45,
                },
            ],
        },
        CurveState {
            key: "gravity",
            label: "Gravity",
            points: vec![
                CurvePoint {
                    time: 0.0,
                    value: 1.0,
                },
                CurvePoint {
                    time: 1.0,
                    value: 1.0,
                },
            ],
        },
        CurveState {
            key: "radius",
            label: "Radius",
            points: vec![
                CurvePoint {
                    time: 0.0,
                    value: 1.0,
                },
                CurvePoint {
                    time: 1.0,
                    value: 0.35,
                },
            ],
        },
    ]
}

fn sample_curve(points: &[CurvePoint], time: f32) -> f32 {
    if points.is_empty() {
        return 0.0;
    }
    let mut sorted = points.to_vec();
    sorted.sort_by(|left, right| left.time.total_cmp(&right.time));
    if time <= sorted[0].time {
        return sorted[0].value;
    }
    for pair in sorted.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if time <= right.time {
            let span = (right.time - left.time).max(f32::EPSILON);
            let alpha = ((time - left.time) / span).clamp(0.0, 1.0);
            return left.value + (right.value - left.value) * alpha;
        }
    }
    sorted.last().map(|point| point.value).unwrap_or(0.0)
}

fn format_hidden_bitmaps(bitmaps: &[u64]) -> String {
    bitmaps
        .iter()
        .map(|bitmap| format!("0x{bitmap:X}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn suggested_slot_count(bitmaps: &[u64]) -> usize {
    let highest = bitmaps
        .iter()
        .filter_map(|bitmap| highest_set_bit(*bitmap))
        .max()
        .map(|bit| bit + 1)
        .unwrap_or(DEFAULT_MASK_SLOTS);
    highest.clamp(DEFAULT_MASK_SLOTS, MAX_MASK_SLOTS)
}

fn highest_set_bit(value: u64) -> Option<usize> {
    (value != 0).then_some((u64::BITS - 1 - value.leading_zeros()) as usize)
}

fn count_uassets(input: &str) -> usize {
    let path = PathBuf::from(input.trim());
    if path.as_os_str().is_empty() {
        0
    } else {
        count_uassets_path(&path)
    }
}

fn count_uassets_path(input: &Path) -> usize {
    if !input.exists() {
        return 0;
    }
    WalkDir::new(input)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("uasset"))
        })
        .count()
}

fn extract_package(package: &Path, output: &Path) -> anyhow::Result<()> {
    let mut config = retoc::Config::default();
    config.aes_keys.insert(
        retoc::FGuid::default(),
        retoc::AesKey::from_str(RIVALS_AES_KEY)?,
    );
    retoc::action_unpack(
        retoc::ActionUnpack {
            utoc: package.to_path_buf(),
            output: output.to_path_buf(),
            verbose: true,
        },
        Arc::new(config),
    )?;
    Ok(())
}
