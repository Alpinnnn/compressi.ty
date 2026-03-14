use std::{fs, path::PathBuf};

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Rect,
    Stroke, TextStyle, Visuals, vec2,
};

// ─── Fanta-Black Monochrome Palette ─────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct ThemeColors {
    /// App chrome / deepest background
    pub bg_base: Color32,
    /// Slightly lifted surface (sidebar, toolbar)
    pub bg_raised: Color32,
    /// Card / panel surface
    pub surface: Color32,
    /// Surface on hover
    pub surface_hover: Color32,
    /// Hairline dividers
    pub border: Color32,
    /// Stronger interactive borders
    pub border_focus: Color32,
    /// Primary accent (warm off-white leans fanta-orange)
    pub accent: Color32,

    /// Highest-contrast text
    pub fg: Color32,
    /// Secondary / caption text
    pub fg_dim: Color32,
    /// Placeholder / disabled text
    pub fg_muted: Color32,
    /// Positive outcomes
    pub positive: Color32,
    /// Caution states
    pub caution: Color32,
    /// Error / destructive
    pub negative: Color32,
}

#[derive(Clone, Copy, Debug)]
pub struct AppTheme {
    pub colors: ThemeColors,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self {
            colors: ThemeColors {
                bg_base: Color32::from_rgb(8, 8, 8),
                bg_raised: Color32::from_rgb(16, 16, 16),
                surface: Color32::from_rgb(22, 22, 22),
                surface_hover: Color32::from_rgb(32, 32, 32),
                border: Color32::from_rgba_premultiplied(255, 255, 255, 10),
                border_focus: Color32::from_rgba_premultiplied(255, 255, 255, 30),
                accent: Color32::from_rgb(255, 138, 62),
                fg: Color32::from_rgb(240, 240, 236),
                fg_dim: Color32::from_rgb(160, 160, 156),
                fg_muted: Color32::from_rgb(90, 90, 86),
                positive: Color32::from_rgb(82, 196, 120),
                caution: Color32::from_rgb(230, 180, 60),
                negative: Color32::from_rgb(220, 72, 72),
            },
        }
    }
}

impl AppTheme {
    // ── Bootstrap ────────────────────────────────────────────────────────

    pub fn apply(&self, ctx: &egui::Context) {
        self.apply_fonts(ctx);

        let mut style = (*ctx.style()).clone();
        style.visuals = Visuals::dark();

        // Background fills
        style.visuals.panel_fill = self.colors.bg_base;
        style.visuals.window_fill = self.colors.bg_raised;
        style.visuals.extreme_bg_color = self.colors.bg_base;

        // Widgets
        style.visuals.widgets.noninteractive.bg_fill = self.colors.bg_raised;
        style.visuals.widgets.noninteractive.bg_stroke.color = self.colors.border;
        style.visuals.widgets.noninteractive.fg_stroke.color = self.colors.fg;
        style.visuals.widgets.inactive.bg_fill = self.colors.surface;
        style.visuals.widgets.inactive.bg_stroke.color = self.colors.border;
        style.visuals.widgets.hovered.bg_fill = self.colors.surface_hover;
        style.visuals.widgets.hovered.bg_stroke.color = self.colors.border_focus;
        style.visuals.widgets.active.bg_fill = self.colors.surface_hover;
        style.visuals.widgets.active.bg_stroke.color = self.colors.border_focus;

        // Selection
        style.visuals.selection.bg_fill = Color32::from_rgba_premultiplied(255, 138, 62, 28);
        style.visuals.selection.stroke = Stroke::new(1.0, self.colors.accent);

        // Global spacing — tight and clean
        style.spacing.item_spacing = vec2(10.0, 10.0);
        style.spacing.button_padding = vec2(16.0, 8.0);

        // Text styles
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(26.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Name("title".into()),
            FontId::new(18.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Name("eyebrow".into()),
            FontId::new(11.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        );

        ctx.set_style(style);
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Linear mix between two colours.
    pub fn mix(&self, a: Color32, b: Color32, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);
        let [ar, ag, ab, aa] = a.to_array();
        let [br, bg, bb, ba] = b.to_array();
        let lerp =
            |x: u8, y: u8| -> u8 { ((x as f32) + ((y as f32) - (x as f32)) * t).round() as u8 };
        Color32::from_rgba_premultiplied(lerp(ar, br), lerp(ag, bg), lerp(ab, bb), lerp(aa, ba))
    }

    pub fn rounded(&self, _radius: u8) -> CornerRadius {
        CornerRadius::ZERO
    }

    pub fn paint_background(&self, painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, CornerRadius::ZERO, self.colors.bg_base);
    }

    // ── Fonts ────────────────────────────────────────────────────────────

    fn apply_fonts(&self, ctx: &egui::Context) {
        let mut fonts = FontDefinitions::default();

        if let Some(font_bytes) = load_google_sans() {
            fonts.font_data.insert(
                "google_sans".to_owned(),
                FontData::from_owned(font_bytes).into(),
            );

            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "google_sans".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                family.insert(0, "google_sans".to_owned());
            }
        }

        if let Some(ionicons) = load_ionicons() {
            fonts
                .font_data
                .insert("ionicons".to_owned(), FontData::from_owned(ionicons).into());

            fonts.families.insert(
                FontFamily::Name("ionicons".into()),
                vec!["ionicons".to_owned()],
            );

            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.push("ionicons".to_owned());
            }
        }

        ctx.set_fonts(fonts);
    }
}

// ─── Font loader ────────────────────────────────────────────────────────────

fn load_google_sans() -> Option<Vec<u8>> {
    let roots = asset_roots();
    let file_names = [
        "GoogleSans-VariableFont_GRAD,opsz,wght.ttf",
        "GoogleSans-Regular.ttf",
        "GoogleSansText-Regular.ttf",
        "GoogleSans-Medium.ttf",
    ];

    for root in roots {
        for file_name in file_names {
            let path = root.join("assets").join("fonts").join(file_name);
            if let Ok(bytes) = fs::read(path) {
                return Some(bytes);
            }
        }
    }

    None
}

fn load_ionicons() -> Option<Vec<u8>> {
    for root in asset_roots() {
        let path = root.join("assets").join("fonts").join("ionicons.ttf");
        if let Ok(bytes) = fs::read(path) {
            return Some(bytes);
        }
    }

    None
}

fn asset_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))];

    if let Ok(current_dir) = std::env::current_dir() {
        if !roots.contains(&current_dir) {
            roots.push(current_dir);
        }
    }

    roots
}
