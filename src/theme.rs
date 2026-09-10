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
        let bg = Color32::from_rgb(19, 21, 26); // #13151a
        let panel = Color32::from_rgb(24, 27, 33); // #181b21
        let accent = Color32::from_rgb(124, 158, 249); // soft indigo
        Self {
            dark: true,
            window_bg: bg,
            panel_bg: panel,
            widget_bg: Color32::from_rgb(30, 34, 41),
            text: Color32::from_rgb(214, 219, 228),
            faint: Color32::from_rgb(122, 129, 145),
            accent,
            accent_soft: Color32::from_rgb(38, 46, 68),
            link: Color32::from_rgb(92, 166, 255),
            code_text: Color32::from_rgb(255, 199, 116),
            code_bg: Color32::from_rgb(28, 28, 34),
            code_fence_bg: Color32::from_rgb(22, 24, 29),
            quote_bar: Color32::from_rgb(86, 118, 196),
            quote_bg: Color32::from_rgb(26, 30, 40),
            stripe: Color32::from_rgb(28, 32, 39),
            hr: Color32::from_rgb(58, 64, 76),
            check: Color32::from_rgb(88, 196, 132),
            danger: Color32::from_rgb(224, 108, 117),
            warning: Color32::from_rgb(229, 192, 123),
            success: Color32::from_rgb(152, 195, 121),
            md_heading: Color32::from_rgb(187, 154, 247),
            md_marker: Color32::from_rgb(97, 110, 143),
            md_em: Color32::from_rgb(139, 175, 158),
            md_strong: Color32::from_rgb(240, 180, 128),
            md_link: Color32::from_rgb(86, 156, 214),
            md_url: Color32::from_rgb(98, 158, 121),
            md_quote: Color32::from_rgb(122, 134, 154),
            md_list: Color32::from_rgb(199, 146, 234),
            md_hr: Color32::from_rgb(92, 99, 112),
            md_html: Color32::from_rgb(86, 182, 194),
            md_fence: Color32::from_rgb(110, 120, 138),
            md_task: Color32::from_rgb(229, 192, 123),
            match_bg: Color32::from_rgba_unmultiplied(180, 140, 30, 110),
        }
    }

    pub fn light() -> Self {
        let accent = Color32::from_rgb(72, 95, 199);
        Self {
            dark: false,
            window_bg: Color32::from_rgb(248, 249, 251),
            panel_bg: Color32::from_rgb(255, 255, 255),
            widget_bg: Color32::from_rgb(238, 240, 244),
            text: Color32::from_rgb(36, 41, 51),
            faint: Color32::from_rgb(110, 118, 132),
            accent,
            accent_soft: Color32::from_rgb(224, 229, 250),
            link: Color32::from_rgb(24, 92, 205),
            code_text: Color32::from_rgb(155, 72, 0),
            code_bg: Color32::from_rgb(244, 244, 246),
            code_fence_bg: Color32::from_rgb(246, 247, 250),
            quote_bar: Color32::from_rgb(114, 140, 214),
            quote_bg: Color32::from_rgb(240, 243, 251),
            stripe: Color32::from_rgb(245, 246, 249),
            hr: Color32::from_rgb(214, 219, 228),
            check: Color32::from_rgb(46, 160, 67),
            danger: Color32::from_rgb(197, 48, 48),
            warning: Color32::from_rgb(176, 106, 0),
            success: Color32::from_rgb(46, 140, 67),
            md_heading: Color32::from_rgb(120, 55, 180),
            md_marker: Color32::from_rgb(140, 148, 162),
            md_em: Color32::from_rgb(64, 125, 96),
            md_strong: Color32::from_rgb(170, 92, 20),
            md_link: Color32::from_rgb(30, 105, 195),
            md_url: Color32::from_rgb(30, 120, 70),
            md_quote: Color32::from_rgb(95, 103, 120),
            md_list: Color32::from_rgb(150, 60, 190),
            md_hr: Color32::from_rgb(160, 166, 178),
            md_html: Color32::from_rgb(40, 135, 150),
            md_fence: Color32::from_rgb(120, 126, 140),
            md_task: Color32::from_rgb(176, 106, 0),
            match_bg: Color32::from_rgba_unmultiplied(255, 213, 79, 120),
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
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.hyperlink_color = self.link;
        visuals.override_text_color = Some(self.text);
        visuals.widgets.inactive.bg_fill = self.widget_bg;
        visuals.widgets.hovered.bg_fill = self.widget_bg.gamma_multiply(1.1);
        visuals.widgets.active.bg_fill = self.accent_soft;
        visuals.widgets.noninteractive.fg_stroke.color = self.text;
        visuals.widgets.inactive.fg_stroke.color = self.text;
        visuals.widgets.hovered.fg_stroke.color = self.text;
        visuals.widgets.active.fg_stroke.color = self.accent;
        let mut style = (*ctx.style()).clone();
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        ctx.set_style(style);
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

/// System bold faces probed in order (the first readable one wins).
const BOLD_FONT_CANDIDATES: &[&str] = &[
    // Windows
    "C:\\Windows\\Fonts\\segoeuib.ttf",
    "C:\\Windows\\Fonts\\arialbd.ttf",
    "C:\\Windows\\Fonts\\tahomabd.ttf",
    // macOS
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/Supplemental/Verdana Bold.ttf",
    // Linux
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
];

/// Register [`BOLD_FAMILY`] (epaint panics on unbound `FontFamily::Name`s, so
/// this must always insert a non-empty stack — the regular fallbacks stay
/// behind the bold face for CJK/emoji coverage).
pub fn register_bold_family(fonts: &mut egui::FontDefinitions) {
    let mut stack: Vec<String> = Vec::new();
    for path in BOLD_FONT_CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert("bold".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            stack.push("bold".to_owned());
            break;
        }
    }
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
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let (bold, regular) = ctx.fonts(|f| {
            (
                f.layout_no_wrap("Wg".to_owned(), bold_font(14.0), Color32::WHITE).size().x,
                f.layout_no_wrap("Wg".to_owned(), egui::FontId::proportional(14.0), Color32::WHITE).size().x,
            )
        });
        assert!(bold > 0.0 && regular > 0.0);
        if BOLD_FONT_CANDIDATES.iter().any(|p| std::fs::read(p).is_ok()) {
            assert_ne!(bold, regular, "a system bold face must actually be used");
        }
    }
}
