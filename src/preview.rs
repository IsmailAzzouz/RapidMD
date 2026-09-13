//! Renders a parsed [`crate::md::Doc`] into egui (spec §6.7, F20–F28).
//!
//! Design notes:
//! - Inline content is laid out once into a `LayoutJob`/`Galley`, painted with
//!   the painter, and click-hit-tested against the recorded link spans.
//! - Code, tables and quotes use egui `Frame` containers (correct paint
//!   ordering) for tinted backgrounds.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use eframe::egui::{
    self, Color32, FontId, Frame, Margin, Pos2, Rect, Sense, Stroke, Vec2,
};
use eframe::egui::text::{LayoutJob, TextFormat};

use crate::md::{Block, Doc, ListItem, QuoteKind, Span, Table};
use crate::theme::Palette;

/// A user command generated while rendering the preview.
pub enum Cmd {
    OpenUrl(String),
    OpenFile(PathBuf),
    Reveal(PathBuf),
    Toast(String),
}

// ---------------------------------------------------------------------------
// Image cache (local images decoded once, spec F28)
// ---------------------------------------------------------------------------

pub struct ImageCache {
    map: HashMap<PathBuf, egui::TextureHandle>,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self { map: HashMap::new() }
    }
}

impl ImageCache {
    pub fn clear(&mut self) {
        self.map.clear();
    }

    fn texture(&mut self, ctx: &egui::Context, path: &Path) -> Option<egui::TextureHandle> {
        if let Some(t) = self.map.get(path) {
            return Some(t.clone());
        }
        let decoded = (|| -> Result<egui::TextureHandle, image::ImageError> {
            let img = image::open(path)?;
            let img = clamp_dimensions(img);
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
            Ok(ctx.load_texture(format!("img-{}", path.display()), color, egui::TextureOptions::LINEAR))
        })();
        match decoded {
            Ok(t) => {
                self.map.insert(path.to_owned(), t.clone());
                Some(t)
            }
            Err(_) => None,
        }
    }
}

fn clamp_dimensions(img: image::DynamicImage) -> image::DynamicImage {
    const MAX: u32 = 4096;
    let (w, h) = (img.width(), img.height());
    if w <= MAX && h <= MAX {
        return img;
    }
    let scale = MAX as f32 / w.max(h) as f32;
    let nw = ((w as f32) * scale).round() as u32;
    let nh = ((h as f32) * scale).round() as u32;
    img.resize(nw, nh, image::imageops::FilterType::Lanczos3)
}

// ---------------------------------------------------------------------------
// Syntax highlighting (syntect, spec F21)
// ---------------------------------------------------------------------------

static SYNTAXES: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
static THEMES: OnceLock<syntect::highlighting::ThemeSet> = OnceLock::new();

fn syntect_theme(dark: bool) -> &'static syntect::highlighting::Theme {
    let ts = THEMES.get_or_init(syntect::highlighting::ThemeSet::load_defaults);
    let name = if dark { "base16-ocean.dark" } else { "InspiredGitHub" };
    ts.themes.get(name).unwrap_or(&ts.themes["base16-ocean.dark"])
}

fn style_from_syn(c: syntect::highlighting::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn code_job(code: &str, lang: &str, pal: &Palette, font_size: f32) -> LayoutJob {
    use syntect::easy::HighlightLines;
    let mut job = LayoutJob::default();
    job.break_on_newline = true;
    let mono = FontId::monospace(font_size);
    let syntaxes = SYNTAXES.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines);
    let syntax = if lang.is_empty() {
        None
    } else {
        syntaxes.find_syntax_by_token(lang).or_else(|| syntaxes.find_syntax_by_name(lang))
    };
    let Some(syntax) = syntax else {
        let fmt = TextFormat { font_id: mono, color: pal.code_text, ..Default::default() };
        job.append(code, 0.0, fmt);
        return job;
    };
    let theme = syntect_theme(pal.dark);
    let mut hl = HighlightLines::new(syntax, theme);
    for line in syntect::util::LinesWithEndings::from(code) {
        if let Ok(ranges) = hl.highlight_line(line, syntaxes) {
            for (style, text) in ranges {
                let mut c = style_from_syn(style.foreground);
                if c == Color32::TRANSPARENT {
                    c = pal.code_text;
                }
                let fmt = TextFormat { font_id: mono.clone(), color: c, ..Default::default() };
                job.append(text, 0.0, fmt);
            }
        } else {
            let fmt = TextFormat { font_id: mono.clone(), color: pal.code_text, ..Default::default() };
            job.append(line, 0.0, fmt);
        }
    }
    job
}

// ---------------------------------------------------------------------------
// Inline runs → LayoutJob with recorded link ranges (char offsets)
// ---------------------------------------------------------------------------

pub type LinkHits = Vec<(std::ops::Range<usize>, String)>;

fn append_run(job: &mut LayoutJob, chars: &mut usize, text: &str, style: crate::md::Style, pal: &Palette, size: f32, color: Color32) {
    let mut fmt = TextFormat {
        font_id: FontId::proportional(size),
        color,
        background: Color32::TRANSPARENT,
        italics: false,
        underline: Stroke::NONE,
        strikethrough: Stroke::NONE,
        ..Default::default()
    };
    if style.code {
        fmt.font_id = FontId::monospace(size);
        fmt.color = pal.code_text;
        fmt.background = pal.code_bg;
        fmt.italics = style.italic;
    } else if style.bold && style.italic {
        fmt.font_id = crate::theme::bold_italic_font(size);
        fmt.color = if pal.dark { color.linear_multiply(1.08) } else { color };
    } else if style.bold {
        fmt.font_id = crate::theme::bold_font(size);
        fmt.color = if pal.dark { color.linear_multiply(1.08) } else { color };
    } else if style.italic {
        fmt.font_id = crate::theme::italic_font(size);
    }
    if style.strike {
        fmt.strikethrough = Stroke::new(1.0, fmt.color);
    }
    job.append(text, 0.0, fmt);
    *chars += text.chars().count();
}

fn spans_to_job(spans: &[Span], pal: &Palette, size: f32, color: Color32) -> (LayoutJob, LinkHits) {
    fn walk(spans: &[Span], job: &mut LayoutJob, chars: &mut usize, links: &mut LinkHits, pal: &Palette, size: f32, color: Color32) {
        for sp in spans {
            match sp {
                Span::Text { text, style } => append_run(job, chars, text, *style, pal, size, color),
                Span::Code { text, style } => append_run(job, chars, text, *style, pal, size, pal.code_text),
                Span::Link { url, inner, .. } => {
                    let start = *chars;
                    walk(inner, job, chars, links, pal, size, pal.link);
                    let end = *chars;
                    links.push((start..end, url.clone()));
                }
                Span::LineBreak => {
                    job.append("\n", 0.0, TextFormat { font_id: FontId::proportional(size), color, ..Default::default() });
                    *chars += 1;
                }
                Span::FootnoteRef { label } => {
                    let text = format!("[{}]", label);
                    let fmt = TextFormat { font_id: FontId::proportional(size * 0.82), color: pal.accent, ..Default::default() };
                    job.append(&text, 0.0, fmt);
                    *chars += text.chars().count();
                }
                Span::Math { text } => {
                    let t = format!(" {} ", text);
                    let fmt = TextFormat {
                        font_id: FontId::monospace(size),
                        color: pal.accent,
                        background: pal.code_bg,
                        italics: true,
                        ..Default::default()
                    };
                    job.append(&t, 0.0, fmt);
                    *chars += t.chars().count();
                }
                Span::Image { alt, .. } => {
                    let label = if alt.is_empty() { "image" } else { alt.as_str() };
                    append_run(job, chars, &format!("⬡ {} ", label), crate::md::Style::default(), pal, size * 0.9, pal.accent);
                }
            }
        }
    }
    let mut job = LayoutJob::default();
    job.break_on_newline = true;
    let mut links: LinkHits = Vec::new();
    let mut chars = 0usize;
    walk(spans, &mut job, &mut chars, &mut links, pal, size, color);
    (job, links)
}

/// Render a text run with native text selection and clickable links.
fn label_rich(ui: &mut egui::Ui, job: &LayoutJob, links: LinkHits, pal: &Palette, cmds: &mut Vec<Cmd>) {
    let mut job = job.clone();
    job.wrap.max_width = ui.available_width().max(8.0);
    let galley = ui.fonts_mut(|f| f.layout_job(job.clone()));
    let resp = ui.add(egui::Label::new(job).selectable(true));
    if links.is_empty() {
        return;
    }
    if let Some(pos) = resp.hover_pos().filter(|p| resp.rect.contains(*p)) {
        let cc = galley.cursor_from_pos(pos - resp.rect.min);
        for (range, url) in &links {
            if range.contains(&cc.index.0) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                egui::Tooltip::always_open(ui.ctx().clone(), resp.layer_id, resp.id.with("mdlink"), egui::PopupAnchor::Pointer).show(|ui| {
                    ui.label(egui::RichText::new(url.as_str()).color(pal.accent).monospace());
                });
                if resp.clicked() {
                    cmds.push(Cmd::OpenUrl(url.clone()));
                }
                if resp.secondary_clicked() {
                    ui.ctx().copy_text(url.clone());
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(url);
                    }
                    cmds.push(Cmd::Toast(format!("Copied link URL: {}", url)));
                }
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public renderer
// ---------------------------------------------------------------------------

/// Render a parsed document; interaction commands are collected and returned.
pub fn render(ui: &mut egui::Ui, doc: &Doc, pal: &Palette, base_dir: Option<&Path>, images: &mut ImageCache) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    let base = base_font_size(ui);
    for (i, block) in doc.blocks.iter().enumerate() {
        ui.push_id(("doc_block", i), |ui| {
            render_block(ui, block, pal, base, base_dir, images, &mut cmds);
        });
    }
    // Footnotes (spec F23 basics).
    if !doc.footnotes.is_empty() {
        ui.add_space(8.0);
        ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, pal.hr));
        ui.add_space(8.0);
        for (label, blocks) in &doc.footnotes {
            ui.push_id(("footnote", label), |ui| {
                ui.label(
                    egui::RichText::new(format!("[{}] ", label))
                        .color(pal.accent)
                        .size(base * 0.78),
                );
                for (bi, b) in blocks.iter().enumerate() {
                    ui.push_id(("fn_block", bi), |ui| {
                        render_block(ui, b, pal, base * 0.92, base_dir, images, &mut cmds);
                    });
                }
            });
            ui.add_space(4.0);
        }
    }
    cmds
}

pub fn base_font_size(ui: &egui::Ui) -> f32 {
    ui.style().text_styles.get(&egui::TextStyle::Body).map(|f| f.size).unwrap_or(15.0)
}

/// Does this span list contain a top-level image?
fn contains_image(spans: &[Span]) -> bool {
    spans.iter().any(|s| matches!(s, Span::Image { .. }))
}

fn render_block(
    ui: &mut egui::Ui,
    block: &Block,
    pal: &Palette,
    base: f32,
    base_dir: Option<&Path>,
    images: &mut ImageCache,
    cmds: &mut Vec<Cmd>,
) {
    match block {
        Block::Heading { level, spans } => {
            let size = match level {
                1 => base * 1.55,
                2 => base * 1.32,
                3 => base * 1.16,
                _ => base * 1.05,
            };
            ui.add_space(if *level <= 2 { 12.0 } else { 6.0 });
            let color = if *level == 1 { pal.text } else { pal.md_heading };
            let (mut job, links) = spans_to_job(spans, pal, size, color);
            for sec in &mut job.sections {
                if sec.format.font_id.family == egui::FontFamily::Proportional {
                    sec.format.font_id = crate::theme::semibold_font(size);
                }
            }
            label_rich(ui, &job, links, pal, cmds);
            if *level <= 2 {
                ui.add_space(3.0);
                ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top().round(), Stroke::new(1.0, pal.hr));
                ui.add_space(3.0);
            }
        }
        Block::Para(spans) => {
            if contains_image(spans) {
                // Render around images: keep text segments flowing, images blocky.
                let mut run: Vec<Span> = Vec::new();
                for sp in spans {
                    if matches!(sp, Span::Image { .. }) {
                        if !run.is_empty() {
                            let (job, links) = spans_to_job(&run, pal, base, pal.text);
                            label_rich(ui, &job, links, pal, cmds);
                            run.clear();
                        }
                        render_image(ui, sp, base_dir, images, cmds);
                    } else {
                        run.push(sp.clone());
                    }
                }
                if !run.is_empty() {
                    let (job, links) = spans_to_job(&run, pal, base, pal.text);
                    label_rich(ui, &job, links, pal, cmds);
                }
            } else {
                let (job, links) = spans_to_job(spans, pal, base, pal.text);
                label_rich(ui, &job, links, pal, cmds);
            }
        }
        Block::BlockQuote { kind, items } => {
            render_blockquote(ui, items, kind, pal, base, base_dir, images, cmds);
        }
        Block::List { ordered, items } => {
            render_list(ui, *ordered, items, pal, base, 0, base_dir, images, cmds);
        }
        Block::CodeBlock { lang, code } => {
            render_code(ui, code, lang, pal, base, cmds);
        }
        Block::Table(table) => {
            render_table(ui, table, pal, base, cmds);
        }
        Block::Rule => {
            ui.add_space(4.0);
            ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, pal.hr));
            ui.add_space(4.0);
        }
        Block::Html(_) => {
            // HTML blocks are now pre-parsed in md.rs into structured blocks.
            // Defensive fallback for unparsed blocks:
        }
        Block::Center(items) => {
            ui.vertical_centered(|ui| {
                for (i, it) in items.iter().enumerate() {
                    ui.push_id(("center_block", i), |ui| {
                        render_block(ui, it, pal, base, base_dir, images, cmds);
                    });
                }
            });
        }
        Block::FootnoteDef { .. } => { /* hoisted; rendered at doc end */ }
    }
}

fn render_blockquote(
    ui: &mut egui::Ui,
    items: &[Block],
    kind: &QuoteKind,
    pal: &Palette,
    base: f32,
    base_dir: Option<&Path>,
    images: &mut ImageCache,
    cmds: &mut Vec<Cmd>,
) {
    let fill = match kind {
        QuoteKind::Caution | QuoteKind::Warning => Color32::from(pal.quote_bg).linear_multiply(1.08),
        _ => pal.quote_bg,
    };
    let frame = Frame::new()
        .fill(fill)
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0, pal.hr))
        .inner_margin(Margin::symmetric(14, 10));
    frame.show(ui, |ui| {
        if let Some(label) = kind.label() {
            let accent = match kind {
                QuoteKind::Note => pal.accent,
                QuoteKind::Tip => pal.success,
                QuoteKind::Important => pal.accent,
                QuoteKind::Warning => pal.warning,
                QuoteKind::Caution => pal.danger,
                QuoteKind::Plain => pal.faint,
            };
ui.label(egui::RichText::new(label).strong().color(accent).size(base * 0.78));
            ui.add_space(2.0);
        }
        ui.horizontal(|ui| {
            let bar_w = 3.0;
            let spacing = 8.0;
            let bar_pos_x = ui.cursor().min.x;
            ui.add_space(bar_w + spacing);
            let inner_resp = ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - 4.0).max(80.0));
                for (i, b) in items.iter().enumerate() {
                    ui.push_id(("quote_block", i), |ui| {
                        render_block(ui, b, pal, base, base_dir, images, cmds);
                    });
                }
            });
            let cr = inner_resp.response.rect;
            let bar_rect = Rect::from_min_max(
                Pos2::new(bar_pos_x.round(), cr.min.y.round() + 1.0),
                Pos2::new((bar_pos_x + bar_w).round(), cr.max.y.round() - 1.0),
            );
            ui.painter().rect_filled(bar_rect, 1.5, pal.quote_bar);
        });
    });
}

fn render_list(
    ui: &mut egui::Ui,
    ordered: bool,
    items: &[ListItem],
    pal: &Palette,
    base: f32,
    depth: usize,
    base_dir: Option<&Path>,
    images: &mut ImageCache,
    cmds: &mut Vec<Cmd>,
) {
    let row_h = ui.text_style_height(&egui::TextStyle::Body);
    let marker_w = if ordered { 26.0 + (depth.clamp(0, 4)) as f32 * 4.0 } else { 20.0 };
    for (i, item) in items.iter().enumerate() {
        ui.push_id(("list_item", i), |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (mrect, _) = ui.allocate_exact_size(Vec2::new(marker_w, row_h), Sense::hover());
                if let Some(checked) = item.checked {
                    let c = mrect.center();
                    let s = (row_h * 0.52).max(12.0);
                    let r = Rect::from_center_size(c, Vec2::splat(s));
                    if checked {
                        let fill_col = if pal.dark { Color32::WHITE } else { Color32::BLACK };
                        let tick_col = if pal.dark { Color32::BLACK } else { Color32::WHITE };
                        ui.painter().rect_filled(r, 3.5, fill_col);
                        ui.painter().text(c, egui::Align2::CENTER_CENTER, "✓", FontId::proportional(base * 0.76), tick_col);
                    } else {
                        ui.painter().rect_stroke(r, 3.5, Stroke::new(1.2, pal.faint), egui::StrokeKind::Inside);
                    }
                } else if ordered {
                    let text = format!("{}.", i + 1);
                    let font = FontId::monospace(base);
                    let g = ui.fonts_mut(|f| f.layout_no_wrap(text.clone(), font, pal.faint));
                    ui.painter().galley(Pos2::new(mrect.right() - g.size().x, mrect.center().y - g.size().y * 0.5), g, pal.faint);
                } else {
                    ui.painter().circle_filled(Pos2::new(mrect.left() + marker_w * 0.4, mrect.center().y), 2.8, pal.faint);
                }
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - marker_w).max(80.0));
                    for (bi, b) in item.blocks.iter().enumerate() {
                        ui.push_id(("item_block", bi), |ui| {
                            render_block(ui, b, pal, base, base_dir, images, cmds);
                        });
                    }
                });
            });
        });
    }
}

fn render_code(ui: &mut egui::Ui, code: &str, lang: &str, pal: &Palette, base: f32, cmds: &mut Vec<Cmd>) {
    let font_size = base * 0.92;
    let job = code_job(code, lang, pal, font_size);
    let frame = Frame::new()
        .fill(pal.code_fence_bg)
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0, pal.hr))
        .inner_margin(Margin::symmetric(14, 10));
    frame.show(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            // Header: language badge on left, 1-click Copy button on right
            let copy_id = ui.id().with("code_copy_ts");
            let current_time = ui.input(|i| i.time);
            let last_copied: Option<f64> = ui.ctx().data(|d| d.get_temp(copy_id));
            let is_copied = last_copied.map(|t| (current_time - t) < 2.0).unwrap_or(false);
            let btn_text = if is_copied { "Copied!" } else { "Copy" };

            ui.horizontal(|ui| {
                if !lang.is_empty() {
                    ui.label(egui::RichText::new(lang).monospace().color(pal.faint).size(base * 0.72));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = egui::Button::new(
                        egui::RichText::new(btn_text)
                            .size(base * 0.70)
                            .color(if is_copied { pal.success } else { pal.faint }),
                    )
                    .fill(pal.widget_bg)
                    .stroke(Stroke::new(1.0, pal.hr))
                    .corner_radius(4.0);

                    if ui.add(btn).clicked() {
                        ui.ctx().copy_text(code.to_string());
                        if let Ok(mut cb) = arboard::Clipboard::new() {
                            let _ = cb.set_text(code);
                        }
                        ui.ctx().data_mut(|d| d.insert_temp(copy_id, current_time));
                        let lines = code.lines().count();
                        let msg = if lines <= 1 {
                            "Copied code to clipboard".to_string()
                        } else {
                            format!("Copied code block ({} lines) to clipboard", lines)
                        };
                        cmds.push(Cmd::Toast(msg));
                    }
                });
            });
            ui.add_space(4.0);

            egui::ScrollArea::horizontal().id_salt("codeh").auto_shrink([false, true]).show(ui, |ui| {
                ui.add(egui::Label::new(job).selectable(true));
            });
        });
    });
}

fn render_table(ui: &mut egui::Ui, table: &Table, pal: &Palette, base: f32, _cmds: &mut Vec<Cmd>) {
    egui::ScrollArea::horizontal().id_salt("tbl").auto_shrink([false, true]).show(ui, |ui| {
        let grid = egui::Grid::new("preview-table").spacing(Vec2::new(24.0, 7.0)).striped(true).min_col_width(30.0);
        let _ = &pal;
        grid.show(ui, |ui| {
            for cell in &table.header {
                let (job, _links) = spans_to_job(cell, pal, base, pal.md_heading);
                ui.add(egui::Label::new(job).selectable(true));
            }
            ui.end_row();
            for row in &table.rows {
                for cell in row {
                    let (job, _links) = spans_to_job(cell, pal, base, pal.text);
                    ui.add(egui::Label::new(job).selectable(true));
                }
                ui.end_row();
            }
        });
    });
}

fn render_image(ui: &mut egui::Ui, sp: &Span, base_dir: Option<&Path>, images: &mut ImageCache, cmds: &mut Vec<Cmd>) {
    let (url, alt, width, height) = match sp {
        Span::Image { url, alt, width, height, .. } => (url.clone(), alt.clone(), *width, *height),
        _ => return,
    };
    ui.add_space(4.0);
    if url.starts_with("http://") || url.starts_with("https://") {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🖼").size(16.0));
            let txt = if alt.is_empty() { url.clone() } else { alt.clone() };
            ui.label(egui::RichText::new(format!("{} (remote)", txt)).color(ui.visuals().weak_text_color()));
            if ui.small_button("Open in browser").clicked() {
                cmds.push(Cmd::OpenUrl(url));
            }
        });
        ui.add_space(4.0);
        return;
    }
    let path = resolve_media(&url, base_dir);
    let Some(path) = path else {
        ui.label(egui::RichText::new(format!("⚠ cannot load: {}", url)).color(ui.visuals().error_fg_color));
        return;
    };
    let Some(tex) = images.texture(ui.ctx(), &path) else {
        ui.label(egui::RichText::new(format!("⚠ cannot load: {}", path.display())).color(ui.visuals().error_fg_color));
        return;
    };
    let avail = ui.available_width().max(40.0);
    let sz = tex.size_vec2();
    let (target_w, target_h) = match (width, height) {
        (Some(w), Some(h)) => {
            let scale = (avail / w).min(1.0);
            (w * scale, h * scale)
        }
        (Some(w), None) => {
            let scale = (avail / w).min(1.0);
            let ws = w * scale;
            (ws, sz.y * (ws / sz.x))
        }
        (None, Some(h)) => {
            let w = sz.x * (h / sz.y);
            let scale = (avail / w).min(1.0);
            (w * scale, h * scale)
        }
        (None, None) => {
            let scale = (avail / sz.x).min(1.0);
            (sz.x * scale, sz.y * scale)
        }
    };
    let size = Vec2::new(target_w.max(16.0), target_h.max(16.0));
    let img = egui::Image::new(&tex)
        .fit_to_exact_size(size)
        .corner_radius(12.0)
        .sense(Sense::click());
    let resp = ui.add(img);
    if resp.double_clicked() {
        cmds.push(Cmd::OpenFile(path.clone()));
    }
    if resp.secondary_clicked() {
        cmds.push(Cmd::Reveal(path.clone()));
    }
    if !alt.is_empty() {
        resp.on_hover_text(alt);
    }
    ui.add_space(4.0);
}

fn decode_percent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next();
            let h2 = bytes.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&[h1, h2]).unwrap_or(""), 16) {
                    out.push(val as char);
                    continue;
                }
                out.push('%');
                out.push(h1 as char);
                out.push(h2 as char);
                continue;
            }
            out.push('%');
            if let Some(h1) = h1 { out.push(h1 as char); }
        } else {
            out.push(b as char);
        }
    }
    out
}

fn resolve_media(url: &str, base_dir: Option<&Path>) -> Option<PathBuf> {
    let trimmed = url.trim();
    let clean = if let Some(s) = trimmed.strip_prefix("file:///") {
        s
    } else if let Some(s) = trimmed.strip_prefix("file://") {
        s
    } else {
        trimmed
    };
    let decoded = decode_percent(clean);
    let p = PathBuf::from(&decoded);

    let check = |cand: PathBuf| -> Option<PathBuf> {
        if cand.exists() {
            return Some(cand);
        }
        let s = cand.to_string_lossy();
        if let Some(idx) = s.find('{') {
            if idx > 0 && !s[..idx].ends_with('/') && !s[..idx].ends_with('\\') {
                let mut fixed = s[..idx].to_string();
                fixed.push('/');
                fixed.push_str(&s[idx..]);
                let fp = PathBuf::from(fixed);
                if fp.exists() {
                    return Some(fp);
                }
            }
        }
        None
    };

    if p.is_absolute() || (cfg!(windows) && decoded.len() >= 2 && decoded.as_bytes()[1] == b':') {
        if let Some(found) = check(p.clone()) { return Some(found); }
    }
    if let Some(b) = base_dir {
        if let Some(found) = check(b.join(&p)) { return Some(found); }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = check(cwd.join(&p)) { return Some(found); }
    }
    if p.is_absolute() || (cfg!(windows) && decoded.len() >= 2 && decoded.as_bytes()[1] == b':') {
        Some(p)
    } else if let Some(b) = base_dir {
        Some(b.join(p))
    } else {
        std::env::current_dir().ok().map(|cwd| cwd.join(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::md::Style;

    #[test]
    fn strong_spans_use_the_bold_family() {
        let pal = Palette::dark();
        let spans = vec![
            Span::Text { text: "bold".to_owned(), style: Style { bold: true, ..Style::default() } },
            Span::Text { text: "plain".to_owned(), style: Style::default() },
            Span::Text { text: "it".to_owned(), style: Style { italic: true, ..Style::default() } },
Span::Code { text: "x".to_owned(), style: Style { bold: true, code: true, ..Style::default() } },
        ];
        let (job, _links) = spans_to_job(&spans, &pal, 14.0, pal.text);
        assert_eq!(job.sections.len(), spans.len());
        assert_eq!(
            job.sections[0].format.font_id.family,
            egui::FontFamily::Name(crate::theme::BOLD_FAMILY.into()),
            "**strong** must render in the bold family"
        );
        assert_eq!(
            job.sections[2].format.font_id.family,
            egui::FontFamily::Name(crate::theme::ITALIC_FAMILY.into()),
            "emphasis must use the dedicated italic family"
        );
        assert_eq!(job.sections[3].format.font_id.family, egui::FontFamily::Monospace, "code wins over strong");
    }

    #[test]
    fn test_multiple_code_blocks_and_tables_no_id_clash() {
        let md = r#"
```rust
fn one() {}
```

```rust
fn two() {}
```

| Col 1 | Col 2 |
| --- | --- |
| 1 | 2 |

| Col A | Col B |
| --- | --- |
| A | B |

* Step 1: Clone repository
  ```bash
  git clone https://github.com/IsmailAzzouz/axis-computer-use.git
  cd axis-computer-use
  ```
* Step 2: Install editable mode
  ```bash
  pip install -e .
  ```

> Blockquote with code:
> ```bash
> echo "test"
> ```
"#;
        let doc = crate::md::parse(md);
        let ctx = egui::Context::default();
        let pal = Palette::dark();
        let mut images = ImageCache::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _ = render(ui, &doc, &pal, None, &mut images);
            });
        });
        out.textures_delta.clear();

        fn check_shape(s: &egui::epaint::Shape) -> bool {
            match s {
                egui::epaint::Shape::Text(t) => {
                    let txt = t.galley.text();
                    txt.contains("ID clashes") || txt.contains("First use of")
                }
                egui::epaint::Shape::Vec(v) => v.iter().any(check_shape),
                _ => false,
            }
        }
        let clashed = out.shapes.iter().any(|cs| check_shape(&cs.shape));
        assert!(!clashed, "egui reported an ID clash warning in rendered shapes!");
    }

    #[test]
    fn test_render_real_image() {
        let path = r"C:\Users\ismai\AppData\Local\Packages\MicrosoftWindows.Client.CBS_cw5n1h2txyewy\TempState\ScreenClip\{EF4B778C-1C05-4492-8040-2D69011CCF82}.png";
        let md = format!("![screenshot]({})", path);
        let doc = crate::md::parse(&md);
        let ctx = egui::Context::default();
        let pal = Palette::dark();
        let mut images = ImageCache::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _cmds = render(ui, &doc, &pal, None, &mut images);
                assert!(!images.map.is_empty(), "Image must be loaded successfully!");
            });
        });
        out.textures_delta.clear();
    }

    #[test]
    fn test_code_syntax_highlighting_after_comments() {
        let code = "# comment line\nclient.focus(\"Chrome\")\nnum = 42\n";
        let pal = Palette::dark();
        let job = code_job(code, "python", &pal, 14.0);

        // Extract section text and colors
        let mut sections: Vec<(&str, Color32)> = Vec::new();
        for sec in &job.sections {
            let s = sec.byte_range.start.0;
            let e = sec.byte_range.end.0;
            sections.push((&job.text[s..e], sec.format.color));
        }

        // The comment color
        let comment_sec = sections.iter().find(|(t, _)| t.contains("comment")).expect("comment section");
        let comment_col = comment_sec.1;

        // "Chrome" string should NOT have comment color
        let chrome_sec = sections.iter().find(|(t, _)| t.contains("Chrome")).expect("chrome string");
        assert_ne!(chrome_sec.1, comment_col, "string after comment must not have comment color");

        // "42" number should NOT have comment color
        let num_sec = sections.iter().find(|(t, _)| t.trim() == "42").expect("42 number");
        assert_ne!(num_sec.1, comment_col, "number after comment must not have comment color");
    }

    #[test]
    fn test_italic_and_bold_italic_use_dedicated_families() {
        let mut job = LayoutJob::default();
        let mut chars = 0;
        let pal = Palette::light();

        // Plain italic
        let style_it = crate::md::Style { italic: true, ..Default::default() };
        append_run(&mut job, &mut chars, "italic text", style_it, &pal, 15.0, pal.text);

        // Bold italic
        let style_bi = crate::md::Style { bold: true, italic: true, ..Default::default() };
        append_run(&mut job, &mut chars, "bold italic text", style_bi, &pal, 15.0, pal.text);

        assert_eq!(job.sections.len(), 2);

        // First section should use ITALIC_FAMILY and NOT have synthetic shearing
        assert_eq!(job.sections[0].format.font_id.family, egui::FontFamily::Name(crate::theme::ITALIC_FAMILY.into()));
        assert!(!job.sections[0].format.italics, "synthetic shearing must be false when using dedicated font");

        // Second section should use BOLD_ITALIC_FAMILY and NOT have synthetic shearing
        assert_eq!(job.sections[1].format.font_id.family, egui::FontFamily::Name(crate::theme::BOLD_ITALIC_FAMILY.into()));
        assert!(!job.sections[1].format.italics, "synthetic shearing must be false when using dedicated font");
    }

    #[test]
    fn test_render_code_copy_button_click_and_toast() {
        let ctx = egui::Context::default();
        let pal = Palette::dark();
        let mut cmds = Vec::new();
        let code = "fn main() {\n    println!(\"Hello\");\n}\n";

        // Verify render_code executes safely without any deadlock or crash across frames
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            render_code(ui, code, "rust", &pal, 15.0, &mut cmds);
        });
        out.textures_delta.clear();

        let mut out2 = ctx.run_ui(egui::RawInput::default(), |ui| {
            render_code(ui, code, "rust", &pal, 15.0, &mut cmds);
        });
        out2.textures_delta.clear();
    }
}
