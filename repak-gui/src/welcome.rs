use crate::main_ui::setup_custom_style;
use crate::ICON;
use eframe::egui;
use eframe::egui::{Color32, Context, RichText};
use log::debug;

struct Contributer {
    name: String,
    link: String,
    description: String,
    platform: String,
}

fn show_contrib(ui: &mut egui::Ui, contributer: Contributer) {
    let name_color = egui::Color32::from_rgb(255, 180, 100); // warm orange;
    let body_color = egui::Color32::WHITE;

    ui.horizontal_wrapped(|ui| {
        ui.colored_label(name_color, "- ");
        ui.hyperlink_to(contributer.name, &contributer.link);

        let url_color = Color32::from_rgb(0,255, 255);
        let link_text = RichText::new(format!("({})", contributer.platform)).color(url_color).strong();
        ui.hyperlink_to(link_text, &contributer.link);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(contributer.description).color(body_color));
    });
    ui.add_space(4.);
}

fn show_support_section(ui: &mut egui::Ui) {
    let contributers: Vec<Contributer> = vec![
        Contributer {
            name: "natimerry".to_string(),
            link: "https://ko-fi.com/natimerry".to_string(),
            description: "I maintain forks of repak and retoc, which convert mods into a format Marvel Rivals can load. I also maintain the mod manager for easy mod installation.".to_string(),
            platform: "Ko-Fi".to_string(),
        },
        Contributer {
            name: "DeathChaosV2".to_string(),
            link: "https://ko-fi.com/deathchaos".to_string(),
            description: "DeathChaos maintains the signature bypass, which enables mods to work. Without him, we would not be able to install mods into the game.".to_string(),
            platform: "Ko-Fi".to_string(),
        },
        Contributer {
            name: "krisanthyme (Otter)".to_string(),
            link: "https://ko-fi.com/krisanthyme".to_string(),
            description: "Wrote the original mesh fixer and also provided the logic using which repak can fix / build mesh mods".to_string(),
            platform: "Ko-Fi".to_string(),
        },
        Contributer {
            name: "amMatt".to_string(),
            link: "https://www.patreon.com/amMatt".to_string(),
            description: "For his IOStorePak tool which allowed us to test and build mods for season 2. Furthermore he created and manages the modding Discord server!".to_string(),
            platform: "Patreon".to_string(),

        },
    ];

    let heading_color = egui::Color32::from_rgb(200, 100, 255); // light purple
    let subheading_color = egui::Color32::from_rgb(100, 200, 255); // light blue
    let body_color = egui::Color32::WHITE;
    let highlight_color = egui::Color32::from_rgb(255, 220, 120); // highlight

    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(30, 30, 30))
        .stroke(egui::Stroke::new(1.0, egui::Color32::DARK_GRAY))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
        .outer_margin(egui::Margin::same(8))
        .show(ui, |mut ui| {
            ui.colored_label(
                heading_color,
                egui::RichText::new("Why Your Support Matters").size(20.0).strong(),
            );

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(
                        "Modding support requires constant effort — from reverse engineering and tooling to updating older mods and ensuring compatibility with game updates. Just in the past two days, over",
                    )
                        .color(body_color),
                );
                ui.label(
                    egui::RichText::new("5,000 lines of code")
                        .color(highlight_color)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("were committed to").color(body_color)
                );
                ui.label(
                    egui::RichText::new("repak-rivals")
                        .color(highlight_color)
                        .italics(),
                );
            });

            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(
                        "This work takes time, skill, and dedication. Your support helps us keep building tools, \
                     maintaining compatibility, and empowering the entire modding community.",
                    )
                        .color(body_color),
                );
            });

            ui.add_space(10.0);
            ui.colored_label(
                subheading_color,
                egui::RichText::new("Meet the Core Contributors:").size(18.0),
            );

            ui.add_space(6.0);

            for contrib in contributers {
                show_contrib(&mut ui, contrib);
            }

            ui.add_space(4.0);
            ui.label("You can bring up this message by clicking on \"Donate\" on the toolbar");
        });
}

pub struct ShowWelcome {}

impl ShowWelcome {
    pub fn welcome_screen(&mut self, ctx: &egui::Context, show_callback: &mut bool) {
        let viewport_options = egui::ViewportBuilder::default()
            .with_title("Support Us")
            .with_icon(ICON.clone())
            .with_inner_size([800., 540.0])
            .with_always_on_top();

        Context::show_viewport_immediate(
            ctx,
            egui::ViewportId::from_hash_of("immediate_viewport"),
            viewport_options,
            |_ui, class| {
                assert!(
                    class == egui::ViewportClass::Immediate,
                    "This egui backend doesn't support multiple viewports"
                );

                setup_custom_style(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Your Support Keeps The Modding Community alive");
                        ui.label("You will see this message only once per version.");

                        ui.horizontal_centered(|ui| {
                            ui.vertical_centered(|ui| {
                                show_support_section(ui);
                            });
                        });
                    });
                });

                if ctx.input(|i| i.viewport().close_requested()) {
                    *show_callback = true;
                    debug!("Closing window");
                }
            },
        );
    }
}
