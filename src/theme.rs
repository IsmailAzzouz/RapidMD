//! Visual theming (spec F29): Light / Dark / System palettes + egui visuals.

use eframe::egui::{self, Color32};

#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        ThemeMode::System
    }
}

/// Whether the OS currently reports a dark color scheme.
pub fn os_is_dark() -> bool {
    matches!(dark_light::detect(), dark_light::Mode::Dark)
}

impl ThemeMode {
    pub fn is_dark(&self) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => os_is_dark(),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }
}

/// Palette used across editor/preview/chrome for a coherent look.
#[derive(Clone)]
pub struct Palette {
    pub dark: bool,
    pub window_bg: Color32,
    pub panel_bg: Color32,
    pub widget_bg: Color32,
    pub text: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub link: Color32,
    pub code_text: Color32,
    pub code_bg: Color32,
    pub code_fence_bg: Color32,
    pub quote_bar: Color32,
    pub quote_bg: Color32,
    pub stripe: Color32,
    pub hr: Color32,
    pub check: Color32,
    pub danger: Color32,
    pub warning: Color32,
    pub success: Color32,
    // Markdown-aware editor token colors (spec F42).
    pub md_heading: Color32,
    pub md_marker: Color32,
    pub md_em: Color32,
    pub md_strong: Color32,
    pub md_link: Color32,
    pub md_url: Color32,
    pub md_quote: Color32,
    pub md_list: Color32,
    pub md_hr: Color32,
    pub md_html: Color32,
    pub md_fence: Color32,
    pub md_task: Color32,
    pub match_bg: Color32,
}

impl Palette {
    pub fn dark() -> Self {
        // Tesla Minimalist Night / OLED Monochrome
        let bg = Color32::from_rgb(12, 13, 15); // #0c0d0f deep OLED canvas
        let panel = Color32::from_rgb(18, 19, 22); // #121316 clean root panel
        let widget = Color32::from_rgb(26, 27, 32); // #1a1b20 rounded pill/card
        let text = Color32::from_rgb(240, 241, 244); // #f0f1f4 crisp off-white
        let faint = Color32::from_rgb(130, 133, 142); // #82858e neutral graphite
        let accent = Color32::from_rgb(255, 255, 255); // pure white contrast
        let accent_soft = Color32::from_rgba_unmultiplied(255, 255, 255, 28);
        Self {
            dark: true,
            window_bg: bg,
            panel_bg: panel,
            widget_bg: widget,
            text,
            faint,
            accent,
            accent_soft,
            link: Color32::from_rgb(148, 180, 219), // refined slate ice blue
            code_text: text,
            code_bg: Color32::from_rgb(24, 25, 29),
            code_fence_bg: Color32::from_rgb(20, 21, 25),
            quote_bar: Color32::from_rgb(100, 102, 112),
            quote_bg: Color32::from_rgb(22, 23, 27),
            stripe: Color32::from_rgb(21, 22, 26),
            hr: Color32::from_rgb(37, 38, 44),
            check: Color32::WHITE,
            danger: Color32::from_rgb(224, 108, 117),
            warning: Color32::from_rgb(212, 162, 89),
            success: Color32::from_rgb(110, 190, 127),
            // Markdown editorial hierarchy (Notion-style monochrome)
            md_heading: Color32::WHITE,
            md_marker: Color32::from_rgb(94, 96, 106),
            md_em: Color32::from_rgb(196, 198, 206),
            md_strong: Color32::WHITE,
            md_link: Color32::from_rgb(148, 180, 219),
            md_url: Color32::from_rgb(114, 116, 128),
            md_quote: Color32::from_rgb(142, 144, 154),
            md_list: Color32::from_rgb(142, 144, 154),
            md_hr: Color32::from_rgb(50, 52, 60),
            md_html: Color32::from_rgb(130, 133, 142),
            md_fence: Color32::from_rgb(94, 96, 106),
            md_task: Color32::from_rgb(142, 144, 154),
            match_bg: Color32::from_rgba_unmultiplied(255, 255, 255, 55),
        }
    }

    pub fn light() -> Self {
        // Tesla / Notion Day Clean Slate
        let bg = Color32::from_rgb(251, 251, 253);
        let panel = Color32::WHITE;
        let widget = Color32::from_rgb(240, 241, 244);
        let text = Color32::from_rgb(17, 17, 19);
        let faint = Color32::from_rgb(115, 117, 127);
        let accent = Color32::from_rgb(17, 17, 19);
        let accent_soft = Color32::from_rgba_unmultiplied(0, 0, 0, 20);
        Self {
            dark: false,
            window_bg: bg,
            panel_bg: panel,
            widget_bg: widget,
            text,
            faint,
            accent,
            accent_soft,
            link: Color32::from_rgb(43, 88, 154),
            code_text: Color32::from_rgb(26, 26, 30),
            code_bg: Color32::from_rgb(238, 240, 243),
            code_fence_bg: Color32::from_rgb(244, 245, 248),
            quote_bar: Color32::from_rgb(160, 162, 170),
            quote_bg: Color32::from_rgb(246, 247, 250),
            stripe: Color32::from_rgb(247, 248, 250),
            hr: Color32::from_rgb(228, 229, 233),
            check: Color32::from_rgb(17, 17, 19),
            danger: Color32::from_rgb(220, 38, 38),
            warning: Color32::from_rgb(180, 83, 9),
            success: Color32::from_rgb(21, 128, 61),
            md_heading: Color32::from_rgb(17, 17, 19),
            md_marker: Color32::from_rgb(156, 158, 168),
            md_em: Color32::from_rgb(60, 62, 68),
            md_strong: Color32::from_rgb(17, 17, 19),
            md_link: Color32::from_rgb(43, 88, 154),
            md_url: Color32::from_rgb(130, 132, 142),
            md_quote: Color32::from_rgb(82, 84, 93),
            md_list: Color32::from_rgb(82, 84, 93),
            md_hr: Color32::from_rgb(208, 210, 215),
            md_html: Color32::from_rgb(115, 117, 127),
            md_fence: Color32::from_rgb(156, 158, 168),
            md_task: Color32::from_rgb(115, 117, 127),
            match_bg: Color32::from_rgba_unmultiplied(0, 0, 0, 45),
        }
    }

    pub fn get(mode: ThemeMode) -> Self {
        if mode.is_dark() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Apply egui visuals so the native widgets match the palette.
    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        visuals.panel_fill = self.panel_bg;
        visuals.window_fill = self.panel_bg;
        visuals.extreme_bg_color = self.widget_bg;
        visuals.faint_bg_color = self.widget_bg;
        visuals.selection.bg_fill = self.accent_soft;
        visuals.selection.stroke = egui::Stroke::new(1.0, if self.dark { Color32::from_white_alpha(160) } else { Color32::from_black_alpha(160) });
        visuals.hyperlink_color = self.link;
        visuals.override_text_color = Some(self.text);

        // Tesla-style sleek rounded corners
        let pill_radius = egui::CornerRadius::same(7);
        let card_radius = egui::CornerRadius::same(10);
        visuals.window_corner_radius = card_radius;
        visuals.menu_corner_radius = pill_radius;

        let stroke_border = egui::Stroke::new(1.0, self.hr);
        visuals.window_stroke = stroke_border;

        visuals.widgets.noninteractive.corner_radius = pill_radius;
        visuals.widgets.inactive.corner_radius = pill_radius;
        visuals.widgets.hovered.corner_radius = pill_radius;
        visuals.widgets.active.corner_radius = pill_radius;
        visuals.widgets.open.corner_radius = pill_radius;

        visuals.widgets.inactive.bg_fill = self.widget_bg;
        visuals.widgets.inactive.bg_stroke = stroke_border;
        visuals.widgets.inactive.fg_stroke.color = self.text;

        visuals.widgets.hovered.bg_fill = if self.dark {
            Color32::from_rgb(38, 40, 47)
        } else {
            Color32::from_rgb(230, 232, 236)
        };
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, if self.dark { Color32::from_white_alpha(45) } else { Color32::from_black_alpha(30) });
        visuals.widgets.hovered.fg_stroke.color = if self.dark { Color32::WHITE } else { Color32::BLACK };

        visuals.widgets.active.bg_fill = self.accent_soft;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, self.accent);
        visuals.widgets.active.fg_stroke.color = self.text;

        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        visuals.widgets.noninteractive.fg_stroke.color = self.text;

        let mut style = (*ctx.global_style()).clone();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(16);

        let mut text_styles = std::collections::BTreeMap::new();
        text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
        text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(13.5));
        text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
        text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        style.text_styles = text_styles;

        // Antialiasing & subpixel crispness
        ctx.tessellation_options_mut(|opt| {
            opt.feathering = true;
            opt.feathering_size_in_pixels = 1.0;
            opt.round_text_to_pixels = true;
        });

        ctx.set_global_style(style);
    }
}

// ---------------------------------------------------------------------------
// Fonts (F94) — egui ships no bold face, so `**strong**` runs are mapped to a
// real system bold font; when none is readable the family degrades to the
// regular stack (the preview keeps a slight color boost instead).
// ---------------------------------------------------------------------------

/// Font family used for strong text (registered by [`register_bold_family`]).
pub const BOLD_FAMILY: &str = "rustdown-bold";

/// `FontId` for bold runs at `size`.
pub fn bold_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(BOLD_FAMILY.into()))
}

/// Register [`BOLD_FAMILY`] (epaint panics on unbound `FontFamily::Name`s, so
/// this must always insert a non-empty stack — the regular fallbacks stay
/// behind the bold face for CJK/emoji coverage).
pub fn register_bold_family(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        "bold".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Bold.ttf"))),
    );
    let mut stack: Vec<String> = vec!["bold".to_owned()];
    stack.extend(fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(egui::FontFamily::Name(BOLD_FAMILY.into()), stack);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_family_is_always_registered() {
        let mut fonts = egui::FontDefinitions::default();
        register_bold_family(&mut fonts);
        let stack = fonts.families.get(&egui::FontFamily::Name(BOLD_FAMILY.into())).expect("bold family");
        assert!(!stack.is_empty(), "the bold family must never be empty");
    }

    #[test]
    fn bold_runs_layout_headless() {
        let mut fonts = egui::FontDefinitions::default();
        register_bold_family(&mut fonts);
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts);
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let (bold, regular) = ctx.fonts_mut(|f| {
            (
                f.layout_no_wrap("Wg".to_owned(), bold_font(14.0), Color32::WHITE).size().x,
                f.layout_no_wrap("Wg".to_owned(), egui::FontId::proportional(14.0), Color32::WHITE).size().x,
            )
        });
        assert!(bold > 0.0 && regular > 0.0);
        assert_ne!(bold, regular, "a system bold face must actually be used");
    }

    #[test]
    fn test_monochrome_palette_dark_and_light() {
        let dark = Palette::dark();
        assert!(dark.dark);
        assert_eq!(dark.accent, Color32::WHITE);
        assert_eq!(dark.md_heading, Color32::WHITE);

        let light = Palette::light();
        assert!(!light.dark);
        assert_eq!(light.accent, Color32::from_rgb(17, 17, 19));
        assert_eq!(light.md_heading, Color32::from_rgb(17, 17, 19));
    }

    #[test]
    fn test_palette_apply_applies_radii_and_visuals() {
        let ctx = egui::Context::default();
        Palette::dark().apply(&ctx);
        assert_eq!(ctx.global_style().visuals.window_corner_radius, egui::CornerRadius::same(10));
        Palette::light().apply(&ctx);
    }
}
