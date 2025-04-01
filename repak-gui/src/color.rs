use eframe::{egui, epaint};
use eframe::egui::{style, Color32, Stroke, Visuals};
use eframe::egui::style::Selection;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy)]
pub struct CustomColor(pub Color32);


impl Serialize for CustomColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let color = self.0;
        let rgba_str = format!("rgba({}, {}, {}, {})", color.r(), color.g(), color.b(), color.a());
        serializer.serialize_str(&rgba_str)
    }
}
impl<'de> Deserialize<'de> for CustomColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let rgba_str: String = Deserialize::deserialize(deserializer)?;

        if !rgba_str.starts_with("rgba(") || !rgba_str.ends_with(")") {
            return Err(serde::de::Error::custom("Invalid rgba format"));
        }

        let values = rgba_str.trim_start_matches("rgba(").trim_end_matches(")").split(',')
            .map(|v| v.trim().parse::<u8>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| serde::de::Error::custom("Invalid rgba value"))?;

        if values.len() != 4 {
            return Err(serde::de::Error::custom("rgba must have exactly 4 values"));
        }

        let (r, g, b, a) = (values[0], values[1], values[2], values[3]);

        Ok(CustomColor(Color32::from_rgba_unmultiplied(r, g, b, a)))
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ColorScheme {
    pub hyperlink_color: CustomColor,
    pub error_fg_color: CustomColor,
    pub warn_fg_color: CustomColor,
    pub text: CustomColor,
    pub overlay: CustomColor,
    pub hovered_widget_color: CustomColor,
    pub active_widget_color: CustomColor,
    pub open_or_inactive_widget_color: CustomColor,
    pub base_color: CustomColor,
    pub code_bg_color: CustomColor,
    pub extreme_bg_color: CustomColor,
    pub selection_bg_fill: CustomColor,
    pub shadow_color: CustomColor,
    pub stroke_color: CustomColor, // effects buttons and progress bars and stuff
    pub selection_stoke_color: CustomColor,
    pub faint_bg_color: CustomColor,

}


impl ColorScheme {
    fn hex_to_rgba(hex: &str) -> Option<Color32> {
        let hex = hex.trim_start_matches('#');

        let len = hex.len();
        match len {
            6 => {
                // RGB format
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color32::from_rgba_unmultiplied(r, g, b, 255))
            }
            8 => {
                // RGBA format
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Color32::from_rgba_unmultiplied(r, g, b, a))
            }
            _ => None, // Invalid format
        }
    }
    fn make_widget_visual(color_scheme: &ColorScheme,
                          old: style::WidgetVisuals,
                          bg_fill: egui::Color32,
    ) -> style::WidgetVisuals {
        style::WidgetVisuals {
            bg_fill,
            weak_bg_fill: bg_fill,
            bg_stroke: egui::Stroke {
                color: color_scheme.overlay.0,
                ..old.bg_stroke
            },
            fg_stroke: egui::Stroke {
                color: color_scheme.text.0,
                ..old.fg_stroke
            },
            ..old
        }
    }
    pub fn apply_egui_style(&self, ctx: &egui::Context) {
        let old = ctx.style().visuals.clone();

        let mut visuals = egui::Visuals {
            override_text_color: Some(self.text.0),
            hyperlink_color: self.hyperlink_color.0,
            faint_bg_color: self.faint_bg_color.0,
            extreme_bg_color: self.extreme_bg_color.0,
            code_bg_color: self.code_bg_color.0,
            warn_fg_color: self.warn_fg_color.0,
            error_fg_color: self.error_fg_color.0,
            window_fill: self.base_color.0,
            panel_fill: self.base_color.0,
            window_stroke: egui::Stroke {
                color: self.overlay.0,
                ..old.window_stroke
            },
            widgets: style::Widgets {
                noninteractive: ColorScheme::make_widget_visual(&self, old.widgets.noninteractive, self.base_color.0),
                inactive: ColorScheme::make_widget_visual(&self, old.widgets.inactive, self.open_or_inactive_widget_color.0),
                hovered: ColorScheme::make_widget_visual(&self, old.widgets.hovered, self.hovered_widget_color.0),
                active: ColorScheme::make_widget_visual(&self, old.widgets.active, self.active_widget_color.0),
                open: ColorScheme::make_widget_visual(&self, old.widgets.open, self.open_or_inactive_widget_color.0),
            },
            selection: style::Selection {
                bg_fill: self.selection_bg_fill.0,
                stroke: egui::Stroke {
                    color: self.selection_stoke_color.0,
                    ..old.selection.stroke
                },
            },
            window_shadow: epaint::Shadow {
                color: self.shadow_color.0,
                ..old.window_shadow
            },
            popup_shadow: epaint::Shadow {
                color: self.shadow_color.0,
                ..old.popup_shadow
            },
            dark_mode: true,
            ..old
        };
        visuals.text_cursor.stroke.color = self.stroke_color.0;
        ctx.set_visuals(visuals);
    }
}
impl Default for ColorScheme {
    fn default() -> Self {
        let mut visuals = Visuals::dark();
        visuals.hyperlink_color = Color32::from_hex("#f71034").expect("Invalid color");
        visuals.text_cursor.stroke.color = Color32::from_hex("#941428").unwrap();
        visuals.selection = Selection {
            bg_fill:  Color32::from_hex("#F1180E3C").unwrap(),
            stroke: Stroke::new(1.0, Color32::from_hex("#000000").unwrap()),
        };
        visuals.window_fill = Color32::from_hex("#FFFFFF").unwrap();
        visuals.panel_fill = Color32::from_hex("#020203").unwrap();
        visuals.faint_bg_color = Color32::from_hex("#08080C").unwrap();
        let unmul =  Color32::from_hex("#F1180E3C").unwrap().to_srgba_unmultiplied();
        let selection_bg_fill = Color32::from_rgba_premultiplied(unmul[0], unmul[1], unmul[2], unmul[3]);
        ColorScheme {
            faint_bg_color: visuals.faint_bg_color.into(),
            stroke_color: visuals.text_cursor.stroke.color.into(),
            hyperlink_color: visuals.hyperlink_color.into(),
            error_fg_color: visuals.error_fg_color.into(),
            warn_fg_color: visuals.warn_fg_color.into(),
            selection_bg_fill: selection_bg_fill.into(),
            selection_stoke_color: visuals.selection.stroke.color.into(),
            text: visuals.text_color().into(),
            overlay: visuals.window_stroke().color.into(),
            hovered_widget_color: visuals.widgets.hovered.bg_fill.into(),
            active_widget_color: visuals.widgets.active.bg_fill.into(),
            open_or_inactive_widget_color: visuals.widgets.open.bg_fill.into(),
            base_color: visuals.panel_fill.into(),
            code_bg_color: visuals.extreme_bg_color.into(), // `extreme_bg_color` is often used for deep backgrounds
            extreme_bg_color: visuals.extreme_bg_color.into(),
            shadow_color: visuals.window_shadow.color.into(),
        }
    }
}
