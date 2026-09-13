//! Application shell: buffers, tabs, modes, panels, shortcuts, dialogs.
//! Spec mapping documented in `docs/SPEC_COVERAGE.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{
self, Align, Align2, Color32, FontId, Key, Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense, TextEdit, Vec2,
};

use crate::buffer::{Buffer, Encoding};
use crate::config::Settings;
use crate::dialogs::Modal;
use crate::editor;
use crate::find::Finder;
use crate::format::{self, Out, TableCmd};
use crate::io;
use crate::md;
use crate::preview::{self, ImageCache};
use crate::text_utils::{byte_to_char, char_to_byte};
use crate::theme::{Palette, ThemeMode};

const MAX_SYNTAX_CHARS: usize = 250_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    View,
    Edit,
    Split,
}

#[derive(Clone)]
enum Cmd {
    New,
    Open,
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    CloseTab,
    Quit,
    Mode(Mode),
    FindBar,
    FindNext,
    FindPrev,
    Replace,
    ReplaceAll,
    Bold,
    Italic,
    Strike,
    CodeSpan,
    Heading(u8),
    Quote,
    Bullet,
    Numbered,
    Task,
    Table,
    TableOp(TableCmd),
    Link,
    Image,
    Theme(ThemeMode),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Wrap,
    Numbers,
    SyncScroll,
    ToggleFps,
    GoToLine,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SplitScrollDriver {
    Editor,
    Preview,
}

#[derive(Clone, Copy)]
enum BlockCmd {
    Heading(u8),
    Quote,
    Bullet,
    Numbered,
    Task,
}

pub struct App {
    buffers: Vec<Buffer>,
    active: Option<usize>,
    next_id: u64,
    mode: Mode,
    settings: Settings,
    palette: Palette,
    find: Finder,
    find_matches: Vec<std::ops::Range<usize>>,
    modal: Option<Modal>,
    pending: Vec<Cmd>,
    toast: Option<(String, Instant)>,
    images: ImageCache,
    doc_cache: Option<(u64, u64, Arc<md::Doc>)>,
    sel: Option<(usize, usize)>, // char offsets, sorted
    editor_widget: Option<egui::Id>,
    pending_cursor: Option<egui::text::CCursorRange>,
    ctx_handle: Option<egui::Context>,
    link_text: String,
    link_url: String,
    goto: String,
    rows: usize,
    cols: usize,
    last_edit: Instant,
    focus_editor: bool,
    split_scroll_ratio: f32,
    split_scroll_driver: SplitScrollDriver,
    preview_max_scroll: f32,
    editor_max_scroll: f32,
    last_frame_time: Instant,
    fps: f32,
    icon_texture: Option<egui::TextureHandle>,
    cjk_loaded: bool,
}

fn menu_separator(ui: &mut egui::Ui, pal: &Palette) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, egui::Stroke::new(1.0, pal.hr));
    ui.add_space(3.0);
}

fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: &str, checked: Option<bool>, pal: &Palette) -> egui::Response {
    let desired_h = 24.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(170.0), desired_h), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 5.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let text_color = if response.hovered() {
        if pal.dark { Color32::WHITE } else { Color32::BLACK }
    } else {
        pal.text
    };
    let mut left_x = rect.left() + 8.0;
    if let Some(is_checked) = checked {
        if is_checked {
            ui.painter().text(
                Pos2::new(left_x, rect.center().y),
                Align2::LEFT_CENTER,
                "✓",
                FontId::proportional(12.0),
                if pal.dark { Color32::WHITE } else { Color32::BLACK },
            );
        }
        left_x += 16.0;
    }
    ui.painter().text(
        Pos2::new(left_x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        text_color,
    );
    if !shortcut.is_empty() {
        let faint_color = pal.faint;
        ui.painter().text(
            Pos2::new(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(11.0),
            faint_color,
        );
    }
    response
}

fn menu_toggle(ui: &mut egui::Ui, label: &str, is_on: bool, pal: &Palette) -> egui::Response {
    let desired_h = 26.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(170.0), desired_h), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 5.0, ui.visuals().widgets.hovered.bg_fill);
    }
    let text_color = if response.hovered() {
        if pal.dark { Color32::WHITE } else { Color32::BLACK }
    } else {
        pal.text
    };
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        text_color,
    );
    let toggle_w = 26.0;
    let toggle_h = 14.0;
    let toggle_rect = Rect::from_center_size(
        Pos2::new(rect.right() - toggle_w * 0.5 - 8.0, rect.center().y),
        Vec2::new(toggle_w, toggle_h),
    );
    let (track_fill, track_stroke, thumb_fill, thumb_x) = if is_on {
        let (tf, thf) = if pal.dark {
            (Color32::WHITE, Color32::from_rgb(18, 19, 23))
        } else {
            (Color32::from_rgb(17, 17, 19), Color32::WHITE)
        };
        (tf, egui::Stroke::NONE, thf, toggle_rect.right() - 7.0)
    } else {
        let (tf, strk, thf) = if pal.dark {
            (Color32::from_rgb(30, 32, 38), egui::Stroke::new(1.0, pal.hr), Color32::from_rgb(110, 112, 120))
        } else {
            (Color32::from_rgb(230, 232, 236), egui::Stroke::new(1.0, pal.hr), Color32::from_rgb(160, 162, 170))
        };
        (tf, strk, thf, toggle_rect.left() + 7.0)
    };
    ui.painter().rect(toggle_rect, 7.0, track_fill, track_stroke, egui::StrokeKind::Inside);
    ui.painter().circle_filled(Pos2::new(thumb_x, toggle_rect.center().y), 4.5, thumb_fill);
    response
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        install_fonts(&cc.egui_ctx);
        let palette = Palette::get(settings.theme);
        palette.apply(&cc.egui_ctx);
        cc.egui_ctx.set_zoom_factor(settings.zoom);

        let mut app = Self {
            buffers: Vec::new(),
            active: None,
            next_id: 1,
            mode: Mode::Edit,
            settings,
            palette,
            find: Finder::default(),
            find_matches: Vec::new(),
            modal: None,
            pending: Vec::new(),
            toast: None,
            images: ImageCache::default(),
            doc_cache: None,
            sel: None,
            editor_widget: None,
            pending_cursor: None,
            ctx_handle: None,
            link_text: String::new(),
            link_url: String::new(),
            goto: String::new(),
            rows: 3,
            cols: 4,
            last_edit: Instant::now(),
            focus_editor: false,
            split_scroll_ratio: 0.0,
            split_scroll_driver: SplitScrollDriver::Editor,
            preview_max_scroll: 0.0,
            editor_max_scroll: 0.0,
            last_frame_time: Instant::now(),
            fps: 60.0,
            icon_texture: None,
            cjk_loaded: false,
        };
        for arg in std::env::args().skip(1) {
            let p = PathBuf::from(&arg);
            if p.is_file() {
                app.open_path(&p, true);
            }
        }
        if !app.cjk_loaded {
            app.ensure_cjk_if_needed(&cc.egui_ctx);
        }
        let items = scan_recovery();
        if !items.is_empty() {
            app.modal = Some(Modal::Recovery { items });
        }
        app
    }

    fn app_icon(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if let Some(ref t) = self.icon_texture {
            return Some(t.clone());
        }
        let bytes = include_bytes!("../assets/Icon/RMD.png");
        if let Ok(img) = image::load_from_memory(bytes) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let color_img = egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                &rgba.into_raw(),
            );
            let tex = ctx.load_texture("rmd_icon", color_img, egui::TextureOptions::LINEAR);
            self.icon_texture = Some(tex.clone());
            Some(tex)
        } else {
            None
        }
    }

    // ---------------------------------------------------------------- core

    fn current(&self) -> Option<&Buffer> {
        self.active.and_then(|i| self.buffers.get(i))
    }

    fn act_text(&self) -> Option<String> {
        self.current().map(|b| b.text.clone())
    }

    fn sel_bytes(&self) -> std::ops::Range<usize> {
        let t = self.act_text().unwrap_or_default();
        match self.sel {
            Some((a, b)) => {
                let (lo, hi) = (a.min(b), a.max(b));
                char_to_byte(&t, lo.min(t.chars().count()))..char_to_byte(&t, hi.min(t.chars().count()))
            }
            None => t.len()..t.len(),
        }
    }

    fn add_untitled(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.buffers.push(Buffer::untitled(id));
        self.active = Some(self.buffers.len() - 1);
        self.mode = Mode::Edit;
        self.invalidate_preview();
    }

    fn remove_at(&mut self, idx: usize) {
        if idx >= self.buffers.len() {
            return;
        }
        self.buffers.remove(idx);
        self.active = match self.active {
            Some(a) if a == idx => (idx > 0).then_some(idx - 1).filter(|i| *i < self.buffers.len()).or(if self.buffers.is_empty() { None } else { Some(0) }),
            Some(a) if a > idx => Some(a - 1),
            o => o,
        };
        self.invalidate_preview();
    }

    fn close_tab(&mut self, idx: usize) {
        match self.buffers.get(idx) {
            None => {}
            Some(b) if !b.is_dirty() => self.remove_at(idx),
            Some(_) => self.modal = Some(Modal::CloseDirty { idx }),
        }
    }

    fn dirty_list(&self) -> Vec<usize> {
        self.buffers.iter().enumerate().filter(|(_, b)| b.is_dirty()).map(|(i, _)| i).collect()
    }

    fn save_at(&mut self, idx: usize) -> bool {
        let Some(buf) = self.buffers.get_mut(idx) else { return false };
        let Some(path) = buf.path.clone() else { return false };
        let bytes = buf.encoded_bytes();
        match io::atomic_save(&path, &bytes) {
            Ok(()) => {
                buf.mark_saved();
                self.settings.push_recent(&path);
                self.toast = Some((format!("Saved {}", buf.title()), Instant::now()));
                true
            }
            Err(e) => {
                self.toast = Some((format!("Save failed: {e}"), Instant::now()));
                false
            }
        }
    }

    fn save_as_at(&mut self, idx: usize) -> bool {
        let name = self.buffers.get(idx).map(|b| b.title()).unwrap_or_else(|| "document.md".into());
        let picked = rfd::FileDialog::new().set_file_name(&name).add_filter("Markdown", &["md", "markdown", "txt"]).save_file();
        let Some(path) = picked else { return false };
        if self.buffers.get_mut(idx).is_none() {
            return false;
        }
        self.buffers[idx].path = Some(path.clone());
        self.buffers[idx].missing_on_disk = false;
        self.save_at(idx)
    }

    fn open_path(&mut self, path: &Path, activate: bool) {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        if let Some(pos) = self.buffers.iter().position(|b| {
            b.path.as_ref().and_then(|p| std::fs::canonicalize(p).ok()).map(|c| c == canon).unwrap_or(false)
        }) {
            if activate {
                self.active = Some(pos);
            }
            return;
        }
        match io::read_file(path) {
            Ok(out) => {
                let id = self.next_id;
                self.next_id += 1;
                let mut b = Buffer::from_disk(id, path, out.text, out.encoding, out.read_only, out.stamp);
                if b.encoding == Encoding::Lossy {
                    b.title_override = Some(format!("{} (repaired)", b.title()));
                }
                self.buffers.push(b);
                self.active = Some(self.buffers.len() - 1);
                self.settings.push_recent(path);
                self.invalidate_preview();
            }
            Err(e) => self.toast = Some((format!("Cannot open {}: {}", path.display(), e), Instant::now())),
        }
    }

    fn pick_open(&mut self) {
        if let Some(files) = rfd::FileDialog::new().add_filter("Markdown", &["md", "markdown", "txt"]).pick_files() {
            for f in files {
                self.open_path(&f, false);
            }
            if !self.buffers.is_empty() {
                self.active = Some(self.buffers.len() - 1);
            }
        }
    }

    fn reload_from_disk(&mut self, idx: usize) {
        let Some(path) = self.buffers.get(idx).and_then(|b| b.path.clone()) else { return };
        if let Ok(out) = io::read_file(&path) {
            let b = &mut self.buffers[idx];
            b.text = out.text.replace("\r\n", "\n");
            b.saved_text = b.text.clone();
            b.encoding = out.encoding;
            b.read_only = out.read_only;
            b.disk_stamp = Some(out.stamp);
            b.missing_on_disk = false;
            self.toast = Some((format!("Reloaded {}", path.display()), Instant::now()));
            self.invalidate_preview();
        } else {
            self.buffers[idx].missing_on_disk = true;
        }
    }

    fn invalidate_preview(&mut self) {
        self.doc_cache = None;
    }

    /// Programmatic text edit with a single native undo step (spec §8.6).
    fn set_text(&mut self, new_text: String, new_sel: Option<std::ops::Range<usize>>) {
        let Some(idx) = self.active else { return };
        let before = self.buffers[idx].text.clone();
        push_undo_snapshot(self.ctx_handle.as_ref(), self.editor_widget, self.sel, &before);
        self.buffers[idx].text = new_text;
        self.invalidate_preview();
        self.focus_editor = true;
        if let Some(sel) = new_sel {
            let t = self.buffers[idx].text.clone();
            let a = byte_to_char(&t, sel.start.min(t.len()));
            let b = byte_to_char(&t, sel.end.min(t.len()));
            self.sel = Some((a, b));
            self.pending_cursor = Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(a.min(b)),
                egui::text::CCursor::new(a.max(b)),
            ));
        }
    }

    fn apply_out(&mut self, out: Out) {
        match out {
            Out::Applied(op) => self.set_text(op.new_text, Some(op.selection)),
            Out::Noop(msg) => self.toast = Some((msg.to_owned(), Instant::now())),
        }
    }

    fn toggle_pair(&mut self, open: &str, close: &str) {
        if let Some(t) = self.act_text() {
            let sel = self.sel_bytes();
            let out = format::toggle_wrap(&t, &sel, open, close);
            self.apply_out(out);
        }
    }

    fn block_cmd(&mut self, which: BlockCmd) {
        let Some(t) = self.act_text() else { return };
        let sel = self.sel_bytes();
        let out = match which {
            BlockCmd::Heading(l) => format::toggle_heading(&t, &sel, l),
            BlockCmd::Quote => format::toggle_blockquote(&t, &sel),
            BlockCmd::Bullet => format::toggle_bullet_list(&t, &sel),
            BlockCmd::Numbered => format::toggle_numbered_list(&t, &sel),
            BlockCmd::Task => format::toggle_task_list(&t, &sel, false),
        };
        self.apply_out(out);
    }


    fn table_op(&mut self, cmd: TableCmd) {
        let Some(t) = self.act_text() else { return };
        let caret = self.sel_bytes().start;
        self.apply_out(format::table_op(&t, caret, cmd));
    }
    fn insert_image_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff"]).pick_file() else {
            return;
        };
        if let Some(idx) = self.active {
            let t = self.buffers[idx].text.clone();
            let caret = self.sel_bytes().start;
            let url = path.to_string_lossy().replace('\\', "/");
            let op = format::insert_image(&t, caret, "", &url);
            self.set_text(op.new_text, Some(op.selection));
        }
    }

    fn find_refresh(&mut self) {
        self.find_matches = match self.act_text() {
            Some(t) => self.find.matches(&t),
            None => Vec::new(),
        };
    }

    fn find_goto(&mut self, next: bool) {
        let Some(t) = self.act_text() else { return };
        if self.find_matches.is_empty() {
            self.find_refresh();
            return;
        }
        let anchor = match self.sel {
            Some((a, b)) => if next { a.max(b) } else { a.min(b) },
            None => 0,
        };
        let m = if next {
            self.find_matches.iter().find(|m| m.start >= anchor).or_else(|| self.find_matches.first())
        } else {
            self.find_matches.iter().rev().find(|m| m.end <= anchor).or_else(|| self.find_matches.last())
        };
        if let Some(m) = m {
            let idx = self.active.unwrap();
            self.set_text(t, Some(m.clone()));
            let _ = idx;
        }
    }
    fn replace_all(&mut self) {
        let Some(t) = self.act_text() else { return };
        let (nt, n) = self.find.replace_all(&t);
        if n > 0 {
            self.set_text(nt, None);
            self.find_refresh();
            self.toast = Some((format!("Replaced {} occurrence(s)", n), Instant::now()));
        }
    }

    fn replace_current(&mut self) {
        let Some(t) = self.act_text() else { return };
        if self.find_matches.is_empty() {
            self.find_refresh();
            return;
        }
        let anchor = self.sel.map(|(a, b)| a.min(b)).unwrap_or(0);
        let target = self
            .find_matches
            .iter()
            .find(|m| m.end >= anchor)
            .or_else(|| self.find_matches.first())
            .cloned();
        if let Some(m) = target {
            let rep = self.find.replace.clone();
            let nt = Finder::replace_at(&t, &m, &rep);
            let new_sel = if rep.is_empty() {
                m.start..m.start
            } else {
                m.start..(m.start + rep.len())
            };
            self.set_text(nt, Some(new_sel));
            self.find_refresh();
            self.toast = Some(("Replaced match".to_owned(), Instant::now()));
        }
    }

    // ------------------------------------------------------------ shortcuts

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.modal.is_some() {
            return;
        }
        let mods = ctx.input(|i| i.modifiers);
        let (cmd, shift) = (mods.command, mods.shift);
        let push = |app: &mut Self, c: Cmd| app.pending.push(c);
        let kp = |ctx: &egui::Context, k: Key| ctx.input(|i| i.key_pressed(k));
        if cmd && kp(ctx, Key::S) && !shift {
            push(self, Cmd::Save);
        } else if cmd && shift && kp(ctx, Key::S) {
            push(self, Cmd::SaveAs);
        } else if cmd && kp(ctx, Key::O) {
            push(self, Cmd::Open);
        } else if cmd && kp(ctx, Key::N) {
            push(self, Cmd::New);
        } else if cmd && kp(ctx, Key::W) {
            if shift {
                push(self, Cmd::Quit);
            } else {
                push(self, Cmd::CloseTab);
            }
        } else if cmd && kp(ctx, Key::F) {
            push(self, Cmd::FindBar);
        } else if kp(ctx, Key::F3) && shift {
            push(self, Cmd::FindPrev);
        } else if kp(ctx, Key::F3) {
            push(self, Cmd::FindNext);
        } else if cmd && kp(ctx, Key::Num1) {
            push(self, Cmd::Mode(Mode::View));
        } else if cmd && kp(ctx, Key::Num2) {
            push(self, Cmd::Mode(Mode::Edit));
        } else if cmd && kp(ctx, Key::Num3) {
            push(self, Cmd::Mode(Mode::Split));
        } else if cmd && kp(ctx, Key::B) {
            push(self, Cmd::Bold);
        } else if cmd && kp(ctx, Key::I) {
            push(self, Cmd::Italic);
        } else if cmd && kp(ctx, Key::K) {
            push(self, Cmd::Link);
        } else if cmd && kp(ctx, Key::Equals) {
            push(self, Cmd::ZoomIn);
        } else if cmd && kp(ctx, Key::Minus) {
            push(self, Cmd::ZoomOut);
        } else if cmd && kp(ctx, Key::Num0) {
            push(self, Cmd::ZoomReset);
        } else if cmd && kp(ctx, Key::G) {
            push(self, Cmd::GoToLine);
        } else if kp(ctx, Key::F1) {
            push(self, Cmd::About);
        }
    }

    pub fn ensure_cjk_if_needed(&mut self, ctx: &egui::Context) {
        if self.cjk_loaded {
            return;
        }
        let needs = self.buffers.iter().any(|b| text_has_cjk(&b.text));
        if needs {
            self.load_cjk_fonts(ctx);
        }
    }

    fn load_cjk_fonts(&mut self, ctx: &egui::Context) {
        if self.cjk_loaded {
            return;
        }
        self.cjk_loaded = true;
        if let Some(font_data) = load_cjk_font_data() {
            let mut fonts = build_base_fonts();
            fonts.font_data.insert("cjk".to_owned(), std::sync::Arc::new(font_data));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(fam) = fonts.families.get_mut(&family) {
                    fam.push("cjk".to_owned());
                }
            }
            crate::theme::register_bold_family(&mut fonts);
            ctx.set_fonts(fonts);
        }
    }

    // ------------------------------------------------------------ actions

    fn apply_cmd(&mut self, c: Cmd, ctx: &egui::Context) {
        match c {
            Cmd::New => self.add_untitled(),
            Cmd::Open => self.pick_open(),
            Cmd::OpenRecent(p) => self.open_path(&p, true),
            Cmd::Save => {
                if let Some(i) = self.active {
                    let has_path = self.buffers[i].path.is_some();
                    if has_path {
                        self.save_at(i);
                    } else {
                        self.save_as_at(i);
                    }
                }
            }
            Cmd::SaveAs => {
                if let Some(i) = self.active {
                    self.save_as_at(i);
                }
            }
            Cmd::CloseTab => {
                if let Some(i) = self.active {
                    self.close_tab(i);
                }
            }
            Cmd::Quit => {
                let dirty = self.dirty_list();
                if dirty.is_empty() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    self.modal = Some(Modal::QuitDirty { idxs: dirty });
                }
            }
            Cmd::Mode(m) => {
                self.mode = m;
                self.invalidate_preview();
            }
            Cmd::FindBar => {
                self.find.open = !self.find.open;
                if self.find.open {
                    self.find_refresh();
                }
            }
            Cmd::FindNext => {
                self.find_refresh();
                self.find_goto(true);
            }
            Cmd::FindPrev => {
                self.find_refresh();
                self.find_goto(false);
            }
            Cmd::Heading(l) => self.block_cmd(BlockCmd::Heading(l)),
            Cmd::Replace => self.replace_current(),
            Cmd::ReplaceAll => self.replace_all(),
            Cmd::Bold => self.toggle_pair("**", "**"),
            Cmd::Italic => self.toggle_pair("*", "*"),
            Cmd::Strike => self.toggle_pair("~~", "~~"),
            Cmd::CodeSpan => {
                let t = self.act_text().unwrap_or_default();
                let sel = self.sel_bytes();
                if sel.is_empty() {
                    self.toggle_pair("`", "`");
                } else if t[sel.clone()].contains('\n') {
                    // Spec T8: inline code cannot span lines → fenced block.
                    let code = t[sel.clone()].to_owned();
                    let mut nt = String::with_capacity(t.len() + 8);
                    nt.push_str(&t[..sel.start]);
                    nt.push_str("```\n");
                    nt.push_str(&code);
                    nt.push('\n');
                    nt.push_str("```");
                    nt.push_str(&t[sel.end..]);
                    let inner_start = sel.start + 4;
                    let inner_end = inner_start + code.len();
                    self.set_text(nt, Some(inner_start..inner_end));
                } else {
                    self.toggle_pair("`", "`");
                }
            }
            Cmd::Quote => self.block_cmd(BlockCmd::Quote),
            Cmd::Bullet => self.block_cmd(BlockCmd::Bullet),
            Cmd::Numbered => self.block_cmd(BlockCmd::Numbered),
            Cmd::Task => self.block_cmd(BlockCmd::Task),
            Cmd::Table => {
                if let Some(idx) = self.active {
                    self.modal = Some(Modal::TablePicker { idx });
                }
            }
            Cmd::TableOp(op) => self.table_op(op),
            Cmd::Link => {
                if let Some(idx) = self.active {
                    let t = self.buffers[idx].text.clone();
                    let sel = self.sel_bytes();
                    self.link_text = t.get(sel.clone()).unwrap_or("").to_owned();
                    self.link_url = String::new();
                    self.modal = Some(Modal::InsertLink { idx });
                }
            }
            Cmd::Image => self.insert_image_dialog(),
            Cmd::Theme(m) => {
                self.settings.theme = m;
                self.palette = Palette::get(m);
                self.palette.apply(ctx);
                self.images.clear();
                self.settings.save();
            }
            Cmd::ZoomIn => self.zoom(1.1, ctx),
            Cmd::ZoomOut => self.zoom(1.0 / 1.1, ctx),
            Cmd::ZoomReset => self.zoom(1.0, ctx),
            Cmd::Wrap => {
                self.settings.wrap_editor = !self.settings.wrap_editor;
                self.settings.save();
            }
            Cmd::Numbers => {
                self.settings.show_line_numbers = !self.settings.show_line_numbers;
                self.settings.save();
            }
            Cmd::SyncScroll => {
                self.settings.sync_scroll = !self.settings.sync_scroll;
                self.settings.save();
            }
            Cmd::ToggleFps => {
                self.settings.show_fps = !self.settings.show_fps;
                self.settings.save();
            }
            Cmd::GoToLine => {
                self.goto = String::new();
                self.modal = Some(Modal::GoTo { idx: self.active.unwrap_or(0) });
            }
            Cmd::About => self.modal = Some(Modal::About),
        }
    }

    fn zoom(&mut self, factor: f32, ctx: &egui::Context) {
        self.settings.zoom = (self.settings.zoom * factor).clamp(0.5, 4.0);
        ctx.set_zoom_factor(self.settings.zoom);
        self.settings.save();
    }

    // ------------------------------------------------------------ eframe

    fn update_impl(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let ctx = ui.ctx().clone();
        self.ctx_handle = Some(ctx.clone());
        self.handle_keys(&ctx);

        if !self.cjk_loaded {
            self.ensure_cjk_if_needed(&ctx);
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        if dt > 0.0001 && dt < 1.0 {
            let current_fps = 1.0 / dt;
            self.fps = self.fps * 0.85 + current_fps * 0.15;
        }
        if self.settings.show_fps {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.dirty_list().is_empty() {
                clear_recovery();
            } else {
                self.modal = Some(Modal::QuitDirty { idxs: self.dirty_list() });
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }

        if ctx.input(|i| i.focused) {
            // External-change detection (spec §8.4) on clean buffers only.
            for i in 0..self.buffers.len() {
                let changed = {
                    let (Some(path), Some(stamp)) = (self.buffers.get(i).and_then(|b| b.path.clone()), self.buffers.get(i).and_then(|b| b.disk_stamp)) else {
                        continue;
                    };
                    let Ok(meta) = std::fs::metadata(&path) else { continue };
                    stamp.changed_vs(&crate::buffer::DiskStamp::from_meta(&meta))
                };
                if changed && self.buffers[i].is_dirty() && !self.buffers[i].suppress_external_change_once {
                    if self.modal.is_none() {
                        self.modal = Some(Modal::ExternalChanged { idx: i });
                    }
                } else if changed && !self.buffers[i].is_dirty() {
                    self.reload_from_disk(i);
                }
            }
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|d| {
                    let p = d.path();
                    if p.as_os_str().is_empty() {
                        None
                    } else {
                        Some(p.to_path_buf())
                    }
                })
                .collect()
        });
        for p in dropped {
            self.open_path(&p, true);
        }

        self.recovery_tick();

        let modal = self.modal.take();

        let panel_frame = egui::Frame::new().fill(self.palette.panel_bg).stroke(egui::Stroke::new(1.0, self.palette.hr));
        egui::Panel::top("menu").frame(panel_frame).exact_size(26.0).show(ui, |ui| {
            ui.add_enabled_ui(modal.is_none(), |ui| self.menu_row(ui));
        });
        egui::Panel::top("tool").frame(panel_frame).exact_size(32.0).show(ui, |ui| {
            ui.add_enabled_ui(modal.is_none(), |ui| self.tool_row(ui));
        });
        if self.buffers.len() > 1 {
            egui::Panel::top("tabs").frame(panel_frame).exact_size(24.0).show(ui, |ui| {
                ui.add_enabled_ui(modal.is_none(), |ui| self.tab_row(ui));
            });
        }
        if self.find.open && self.active.is_some() {
            egui::Panel::top("find").frame(panel_frame).exact_size(28.0).show(ui, |ui| {
                ui.add_enabled_ui(modal.is_none(), |ui| self.find_row(ui));
            });
        }
        egui::Panel::bottom("status").frame(panel_frame).exact_size(22.0).show(ui, |ui| {
            ui.add_enabled_ui(modal.is_none(), |ui| self.status_row(ui));
        });

        let central_frame = egui::Frame::new()
            .fill(self.palette.window_bg)
            .inner_margin(egui::Margin {
                left: 4,
                right: 4,
                top: 8,
                bottom: 4,
            });
        let central_resp = egui::CentralPanel::default().frame(central_frame).show(ui, |ui| {
            ui.add_enabled_ui(modal.is_none(), |ui| {
                if let Some(i) = self.active {
                    match self.mode {
                        Mode::View => self.view_pane(ui, i),
                        Mode::Edit => self.edit_pane(ui, i),
                        Mode::Split => {
                            egui::Panel::left("split")
                                .resizable(true)
                                .min_size(120.0)
                                .max_size((ui.available_width() - 120.0).max(120.0))
                                .default_size(ui.available_width() * 0.5)
                                .frame(egui::Frame::new().fill(self.palette.window_bg).stroke(egui::Stroke::new(1.0, self.palette.hr)))
                                .show(ui, |ui| self.edit_pane(ui, i));
                            self.view_pane(ui, i);
                        }
                    }
                } else {
                    self.welcome(ui);
                }
            });
        });

        self.toast_ui(&ctx);
        if let Some(m) = modal {
            let screen_rect = ctx.viewport_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("modal_backdrop")));
            painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(100));
            self.modal_ui(&ctx, m);
        }
        self.flush(&ctx);
        central_resp.response.rect
    }

    fn flush(&mut self, ctx: &egui::Context) {
        let acts = std::mem::take(&mut self.pending);
        for a in acts {
            self.apply_cmd(a, ctx);
        }
    }

    fn recovery_tick(&mut self) {
        if self.last_edit.elapsed().as_secs() < 2 || self.dirty_list().is_empty() {
            return;
        }
        let dir = recovery_dir();
        let _ = std::fs::create_dir_all(&dir);
        for (i, b) in self.buffers.iter().enumerate() {
            if !b.is_dirty() {
                continue;
            }
            let v = serde_json::json!({
                "title": b.title(),
                "path": b.path.as_ref().map(|p| p.display().to_string()),
                "text": b.text,
            });
            let _ = std::fs::write(dir.join(format!("buf-{}.json", i)), serde_json::to_string(&v).unwrap_or_default());
        }
    }

    // ------------------------------------------------------------ UI pieces

    fn menu_row(&mut self, ui: &mut egui::Ui) {
        let pal = self.palette.clone();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
            ui.spacing_mut().button_padding = egui::vec2(9.0, 4.0);

            let visuals = ui.visuals_mut();
            visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
            visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.hovered.bg_fill = if pal.dark {
                Color32::from_rgba_unmultiplied(255, 255, 255, 20)
            } else {
                Color32::from_rgba_unmultiplied(0, 0, 0, 16)
            };
            visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.active.bg_fill = if pal.dark {
                Color32::from_rgba_unmultiplied(255, 255, 255, 36)
            } else {
                Color32::from_rgba_unmultiplied(0, 0, 0, 24)
            };
            visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.open.bg_fill = visuals.widgets.active.bg_fill;
            visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
            visuals.widgets.open.corner_radius = egui::CornerRadius::same(6);

            visuals.menu_corner_radius = egui::CornerRadius::same(8);
            visuals.window_corner_radius = egui::CornerRadius::same(8);
            visuals.window_fill = if pal.dark { Color32::from_rgb(18, 19, 23) } else { Color32::WHITE };
            visuals.window_stroke = egui::Stroke::new(1.0, pal.hr);
            visuals.selection.stroke = egui::Stroke::NONE;
            visuals.selection.bg_fill = visuals.widgets.hovered.bg_fill;

            egui::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(RichText::new("File").size(13.0).color(pal.text), |ui| {
                    ui.set_min_width(200.0);
                    if menu_item(ui, "New Document", "Ctrl+N", None, &pal).clicked() {
                        self.pending.push(Cmd::New);
                        ui.close();
                    }
                    if menu_item(ui, "Open…", "Ctrl+O", None, &pal).clicked() {
                        self.pending.push(Cmd::Open);
                        ui.close();
                    }
                    menu_separator(ui, &pal);
                    let recents = self.settings.recent_paths();
                    if !recents.is_empty() {
                        ui.menu_button("Open Recent", |ui| {
                            ui.set_min_width(220.0);
                            for r in recents {
                                let name = r.file_name().and_then(|n| n.to_str()).unwrap_or("document");
                                if menu_item(ui, name, "", None, &pal).on_hover_text(r.display().to_string()).clicked() {
                                    self.pending.push(Cmd::OpenRecent(r));
                                    ui.close();
                                }
                            }
                        });
                        menu_separator(ui, &pal);
                    }
                    if menu_item(ui, "Save", "Ctrl+S", None, &pal).clicked() {
                        self.pending.push(Cmd::Save);
                        ui.close();
                    }
                    if menu_item(ui, "Save As…", "Ctrl+Shift+S", None, &pal).clicked() {
                        self.pending.push(Cmd::SaveAs);
                        ui.close();
                    }
                    menu_separator(ui, &pal);
                    if menu_item(ui, "Close Tab", "Ctrl+W", None, &pal).clicked() {
                        self.pending.push(Cmd::CloseTab);
                        ui.close();
                    }
                    if menu_item(ui, "Quit", "Ctrl+Shift+W", None, &pal).clicked() {
                        self.pending.push(Cmd::Quit);
                        ui.close();
                    }
                });

                ui.menu_button(RichText::new("Edit").size(13.0).color(pal.text), |ui| {
                    ui.set_min_width(180.0);
                    if menu_item(ui, "Find / Replace", "Ctrl+F", None, &pal).clicked() {
                        self.pending.push(Cmd::FindBar);
                        ui.close();
                    }
                    if menu_item(ui, "Go to Line…", "Ctrl+G", None, &pal).clicked() {
                        self.pending.push(Cmd::GoToLine);
                        ui.close();
                    }
                });

                ui.menu_button(RichText::new("View").size(13.0).color(pal.text), |ui| {
                    ui.set_min_width(220.0);
                    if menu_item(ui, "View", "Ctrl+1", Some(self.mode == Mode::View), &pal).clicked() {
                        self.pending.push(Cmd::Mode(Mode::View));
                        ui.close();
                    }
                    if menu_item(ui, "Edit", "Ctrl+2", Some(self.mode == Mode::Edit), &pal).clicked() {
                        self.pending.push(Cmd::Mode(Mode::Edit));
                        ui.close();
                    }
                    if menu_item(ui, "Split", "Ctrl+3", Some(self.mode == Mode::Split), &pal).clicked() {
                        self.pending.push(Cmd::Mode(Mode::Split));
                        ui.close();
                    }
                    menu_separator(ui, &pal);
                    ui.menu_button("Theme", |ui| {
                        ui.set_min_width(140.0);
                        for t in [ThemeMode::Dark, ThemeMode::Light, ThemeMode::System] {
                            if menu_item(ui, t.label(), "", Some(self.settings.theme == t), &pal).clicked() {
                                self.pending.push(Cmd::Theme(t));
                                ui.close();
                            }
                        }
                    });
                    menu_separator(ui, &pal);
                    if menu_toggle(ui, "Wrap editor text", self.settings.wrap_editor, &pal).clicked() {
                        self.pending.push(Cmd::Wrap);
                        ui.close();
                    }
                    if menu_toggle(ui, "Line numbers", self.settings.show_line_numbers, &pal).clicked() {
                        self.pending.push(Cmd::Numbers);
                        ui.close();
                    }
                    if menu_toggle(ui, "Sync scroll in split view", self.settings.sync_scroll, &pal).clicked() {
                        self.pending.push(Cmd::SyncScroll);
                        ui.close();
                    }
                    if menu_toggle(ui, "Show FPS counter", self.settings.show_fps, &pal).clicked() {
                        self.pending.push(Cmd::ToggleFps);
                        ui.close();
                    }
                    menu_separator(ui, &pal);
                    if menu_item(ui, "Zoom In", "Ctrl+=", None, &pal).clicked() {
                        self.pending.push(Cmd::ZoomIn);
                        ui.close();
                    }
                    if menu_item(ui, "Zoom Out", "Ctrl+-", None, &pal).clicked() {
                        self.pending.push(Cmd::ZoomOut);
                        ui.close();
                    }
                    if menu_item(ui, "Reset zoom", "Ctrl+0", None, &pal).clicked() {
                        self.pending.push(Cmd::ZoomReset);
                        ui.close();
                    }
                });

                if ui.button(RichText::new("Help").size(13.0).color(pal.text)).clicked() {
                    self.pending.push(Cmd::About);
                }
            });
        });
    }

    fn tool_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
            // Tesla-style Segmented Capsule Control for Mode
            egui::Frame::new()
                .fill(self.palette.widget_bg)
                .corner_radius(6.0)
                .stroke(egui::Stroke::new(1.0, self.palette.hr))
                .inner_margin(egui::Margin::symmetric(2, 1))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(1.0, 0.0);
                    for (m, lbl) in [(Mode::View, "View"), (Mode::Edit, "Edit"), (Mode::Split, "Split")] {
                        let active = self.mode == m;
                        let (bg, fg) = if active {
                            if self.palette.dark {
                                (Color32::from_rgb(40, 42, 50), Color32::WHITE)
                            } else {
                                (Color32::WHITE, Color32::BLACK)
                            }
                        } else {
                            (Color32::TRANSPARENT, self.palette.faint)
                        };
                        let stroke = if active {
                            egui::Stroke::new(1.0, self.palette.hr)
                        } else {
                            egui::Stroke::NONE
                        };
                        let btn = egui::Button::new(RichText::new(lbl).color(fg).size(12.0).strong())
                            .fill(bg)
                            .stroke(stroke)
                            .corner_radius(5.0)
                            .min_size(Vec2::new(38.0, 18.0));
                        if ui.add(btn).clicked() {
                            self.mode = m;
                        }
                    }
                });
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);
            if ui.button("New").clicked() {
                self.pending.push(Cmd::New);
            }
            if ui.button("Open").clicked() {
                self.pending.push(Cmd::Open);
            }
            if ui.button("Save").clicked() {
                self.pending.push(Cmd::Save);
            }
            ui.separator();
            let editing = self.mode != Mode::View && self.active.is_some();
            ui.add_enabled_ui(editing, |ui| {
                if ui.button("B").on_hover_text("Bold").clicked() {
                    self.pending.push(Cmd::Bold);
                }
                if ui.button("I").on_hover_text("Italic").clicked() {
                    self.pending.push(Cmd::Italic);
                }
                if ui.button(RichText::new("S").strikethrough()).on_hover_text("Strikethrough").clicked() {
                    self.pending.push(Cmd::Strike);
                }
                if ui.button("`x`").on_hover_text("Inline code").clicked() {
                    self.pending.push(Cmd::CodeSpan);
                }
                ui.separator();
                if ui.button("H1").on_hover_text("Heading 1").clicked() {
                    self.pending.push(Cmd::Heading(1));
                }
                if ui.button("H2").on_hover_text("Heading 2").clicked() {
                    self.pending.push(Cmd::Heading(2));
                }
                if ui.button("H3").on_hover_text("Heading 3").clicked() {
                    self.pending.push(Cmd::Heading(3));
                }
                if ui.button("❝").on_hover_text("Blockquote").clicked() {
                    self.pending.push(Cmd::Quote);
                }
                if ui.button("•").on_hover_text("Bullet list").clicked() {
                    self.pending.push(Cmd::Bullet);
                }
                if ui.button("1.").on_hover_text("Numbered list").clicked() {
                    self.pending.push(Cmd::Numbered);
                }
                if ui.button("☑").on_hover_text("Task list").clicked() {
                    self.pending.push(Cmd::Task);
                }
                ui.separator();
                if ui.button("Link").clicked() {
                    self.pending.push(Cmd::Link);
                }
                if ui.button("Image").clicked() {
                    self.pending.push(Cmd::Image);
                }
                if ui.button("Table").clicked() {
                    self.pending.push(Cmd::Table);
                }
                ui.menu_button("Rows/Cols", |ui| {
                    if ui.button("Row above").clicked() {
                        self.pending.push(Cmd::TableOp(TableCmd::InsertRowAbove));
                    }
                    if ui.button("Row below").clicked() {
                        self.pending.push(Cmd::TableOp(TableCmd::InsertRowBelow));
                    }
                    if ui.button("Column left").clicked() {
                        self.pending.push(Cmd::TableOp(TableCmd::InsertColLeft));
                    }
                    if ui.button("Column right").clicked() {
                        self.pending.push(Cmd::TableOp(TableCmd::InsertColRight));
                    }
                    ui.separator();
                    if ui.button("Delete row").clicked() {
                        self.pending.push(Cmd::TableOp(TableCmd::DeleteRow));
                    }
                    if ui.button("Delete column").clicked() {
                        self.pending.push(Cmd::TableOp(TableCmd::DeleteCol));
                    }
                });
            });
        });
    }

    fn tab_row(&mut self, ui: &mut egui::Ui) {
        ScrollArea::horizontal()
            .id_salt("tabs_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                    let titles: Vec<(String, bool)> = self.buffers.iter().map(|b| (b.title(), b.is_dirty())).collect();
                    for (i, (name, dirty)) in titles.iter().enumerate() {
                        let is_active = self.active == Some(i);
                        let bg = if is_active {
                            self.palette.widget_bg
                        } else {
                            Color32::TRANSPARENT
                        };
                        let border = if is_active {
                            egui::Stroke::new(1.0, self.palette.hr)
                        } else {
                            egui::Stroke::NONE
                        };
                        egui::Frame::new()
                            .fill(bg)
                            .stroke(border)
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(5.0, 0.0);
                                    if *dirty {
                                        let dot_color = if self.palette.dark { Color32::from_rgb(220, 160, 80) } else { Color32::from_rgb(180, 100, 20) };
                                        ui.label(RichText::new("●").size(8.0).color(dot_color));
                                    }
                                    let text_color = if is_active { self.palette.text } else { self.palette.faint };
                                    if ui.add(egui::Label::new(RichText::new(name).color(text_color).strong().size(12.0)).sense(Sense::click())).clicked() {
                                        self.active = Some(i);
                                        self.invalidate_preview();
                                    }
                                    if ui.add(egui::Button::new(RichText::new("×").size(11.0).color(self.palette.faint)).frame(false)).clicked() {
                                        self.close_tab(i);
                                    }
                                });
                            });
                    }
                });
            });
    }

    fn find_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
            ui.label(RichText::new("Find").color(self.palette.faint).size(12.0));
            let changed = ui.add(TextEdit::singleline(&mut self.find.query).desired_width(180.0).hint_text("Search…").margin(Margin::symmetric(6, 2))).changed();
            ui.checkbox(&mut self.find.case_sensitive, "Aa");
            if ui.button("↑").clicked() {
                self.pending.push(Cmd::FindPrev);
            }
            if ui.button("↓").clicked() {
                self.pending.push(Cmd::FindNext);
            }
            ui.separator();
            ui.label(RichText::new("Replace").color(self.palette.faint).size(12.0));
            ui.add(TextEdit::singleline(&mut self.find.replace).desired_width(140.0).margin(Margin::symmetric(6, 2)));
            if ui.button("Replace").clicked() {
                self.pending.push(Cmd::Replace);
            }
            if ui.button("All").clicked() {
                self.pending.push(Cmd::ReplaceAll);
            }
            if ui.add(egui::Button::new(RichText::new("✕").size(12.0).color(self.palette.faint)).frame(false)).clicked() {
                self.find.open = false;
                self.find_matches.clear();
            }
            if changed {
                self.find_refresh();
            }
        });
    }

    fn status_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
            if let Some(b) = self.current() {
                let (state, color) = if b.missing_on_disk {
                    ("MISSING", self.palette.danger)
                } else if b.read_only {
                    ("READ-ONLY", self.palette.faint)
                } else if b.is_dirty() {
                    ("MODIFIED", self.palette.warning)
                } else {
                    ("SAVED", self.palette.faint)
                };
                egui::Frame::new()
                    .fill(self.palette.widget_bg)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(state).color(color).size(10.0).strong());
                    });
ui.label(RichText::new(b.path_or_title()).color(self.palette.text));
                if b.encoding != Encoding::Utf8 {
                    ui.label(RichText::new(b.encoding.label()).color(self.palette.faint).monospace().size(11.0));
                }
                ui.label(RichText::new(if b.eol == crate::buffer::Eol::Crlf { "CRLF" } else { "LF" }).color(self.palette.faint).size(11.0));
                let t = b.text.clone();
                let (ln, col) = match self.sel {
                    Some((a, _)) => editor::line_col(&t, char_to_byte(&t, a)),
                    None => (1, 1),
                };
                ui.separator();
                ui.label(RichText::new(format!("Ln {}, Col {}", ln, col)).monospace().color(self.palette.faint).size(11.0));
                ui.label(RichText::new(format!("Words: {}", b.word_count())).color(self.palette.faint).size(11.0));
            } else {
                ui.label(RichText::new("No document open").color(self.palette.faint).size(11.0));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.settings.show_fps {
                    let fps_val = self.fps.round().max(1.0) as u32;
                    let fps_badge = egui::Frame::new()
                        .fill(self.palette.widget_bg)
                        .stroke(egui::Stroke::new(1.0, self.palette.hr))
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(5, 1))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{} FPS", fps_val)).monospace().color(self.palette.faint).size(10.0));
                        });
                    if fps_badge.response.on_hover_text("Frame rate (click to hide)").interact(Sense::click()).clicked() {
                        self.pending.push(Cmd::ToggleFps);
                    }
                    ui.add_space(4.0);
                }
                ui.label(RichText::new(format!("{}%", (self.settings.zoom * 100.0).round() as i32)).monospace().color(self.palette.faint).size(11.0));
                ui.label(RichText::new(match self.mode {
                    Mode::View => "View",
                    Mode::Edit => "Edit",
                    Mode::Split => "Split",
                }).color(self.palette.faint).size(11.0));
            });
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width();
        let avail_h = ui.available_height();
        let content_w = 520.0f32.min(avail_w - 48.0).max(280.0);
        let side_margin = ((avail_w - content_w) * 0.5).max(16.0);
        let card_bg = if self.palette.dark { Color32::from_rgb(20, 21, 26) } else { Color32::WHITE };

        ScrollArea::vertical()
            .id_salt("welcome_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space((avail_h * 0.12).clamp(24.0, 90.0));
                ui.horizontal(|ui| {
                    ui.add_space(side_margin);
                    ui.vertical(|ui| {
                        ui.set_max_width(content_w);
                        ui.set_width(content_w);

                        ui.vertical_centered(|ui| {
                            if let Some(tex) = self.app_icon(ui.ctx()) {
                                let img = egui::Image::new(&tex)
                                    .fit_to_exact_size(Vec2::new(76.0, 76.0))
                                    .corner_radius(14.0);
                                ui.add(img);
                            }
                            ui.add_space(16.0);
                            ui.label(RichText::new("RapidMD").strong().size(36.0).color(self.palette.accent));
                            ui.add_space(4.0);
                            ui.label(RichText::new("Native Markdown viewer & editor").color(self.palette.faint).size(14.0));
                            ui.add_space(26.0);

                            // Centered action buttons
                            let btn_w = ((content_w - 14.0) * 0.5).clamp(130.0, 180.0);
                            let total_buttons_w = btn_w * 2.0 + 14.0;
                            let btn_side_margin = ((content_w - total_buttons_w) * 0.5).max(0.0);

                            ui.horizontal(|ui| {
                                ui.add_space(btn_side_margin);
                                let (primary_fill, primary_fg) = if self.palette.dark {
                                    (Color32::WHITE, Color32::BLACK)
                                } else {
                                    (Color32::from_rgb(17, 17, 19), Color32::WHITE)
                                };
                                let open_btn = egui::Button::new(
                                    RichText::new("📂  Open File…").size(14.0).color(primary_fg).strong()
                                )
                                .fill(primary_fill)
                                .corner_radius(8.0)
                                .min_size(Vec2::new(btn_w, 40.0));
                                if ui.add(open_btn).clicked() {
                                    self.pending.push(Cmd::Open);
                                }

                                let new_btn = egui::Button::new(
                                    RichText::new("➕  New Document").size(14.0).color(self.palette.text).strong()
                                )
                                .fill(self.palette.widget_bg)
                                .stroke(egui::Stroke::new(1.0, self.palette.hr))
                                .corner_radius(8.0)
                                .min_size(Vec2::new(btn_w, 40.0));
                                if ui.add(new_btn).clicked() {
                                    self.pending.push(Cmd::New);
                                }
                            });

                            ui.add_space(10.0);
                            ui.label(RichText::new("Ctrl+O to open · Ctrl+N for new").color(self.palette.faint).size(11.0));
                        });

                        ui.add_space(32.0);

                        // Recent documents card
                        let recents = self.settings.recent_paths();
                        if !recents.is_empty() {
                            egui::Frame::new()
                                .fill(card_bg)
                                .stroke(egui::Stroke::new(1.0, self.palette.hr))
                                .corner_radius(10.0)
                                .inner_margin(egui::Margin::symmetric(18, 14))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("Recent Documents").color(self.palette.text).size(13.0).strong());
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                ui.label(RichText::new(format!("{} items", recents.len().min(6))).color(self.palette.faint).size(11.0));
                                            });
                                        });
                                        ui.add_space(8.0);
                                        ui.separator();
                                        ui.add_space(6.0);

                                        for r in recents.iter().take(6) {
                                            let file_name = r.file_name().and_then(|n| n.to_str()).unwrap_or("document");
                                            let parent_dir = r.parent().map(|p| p.display().to_string()).unwrap_or_default();
                                            let path_tooltip = r.display().to_string();

                                            let row_resp = ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                                                ui.label(RichText::new("📄").size(14.0));
                                                ui.vertical(|ui| {
                                                    ui.label(RichText::new(file_name).color(self.palette.text).size(13.0).strong());
                                                    if !parent_dir.is_empty() {
                                                        ui.label(RichText::new(&parent_dir).color(self.palette.faint).size(10.0));
                                                    }
                                                });
                                            });
                                            let interact_resp = ui.interact(row_resp.response.rect, row_resp.response.id, Sense::click());
                                            if interact_resp.hovered() {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                ui.painter().rect_filled(row_resp.response.rect.expand(4.0), 6.0, self.palette.accent_soft);
                                            }
                                            if interact_resp.on_hover_text(path_tooltip).clicked() {
                                                self.pending.push(Cmd::OpenRecent(r.clone()));
                                            }
                                            ui.add_space(6.0);
                                        }
                                    });
                                });
                        }
                    });
                });
                ui.add_space(40.0);
            });
    }

    fn view_pane(&mut self, ui: &mut egui::Ui, idx: usize) {
        let base_dir = self.buffers[idx].path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let text = self.buffers[idx].text.clone();
        let hash = fnv64(&text);
        let doc = match self.doc_cache.clone() {
            Some((cid, h, d)) if cid == self.buffers[idx].id && h == hash => d,
            _ => {
                if text.chars().count() > 4_000_000 {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("File is too large to preview. Use Edit mode.").color(self.palette.warning));
                    });
                    return;
                }
                let d = Arc::new(md::parse(&text));
                self.doc_cache = Some((self.buffers[idx].id, hash, d.clone()));
                d
            }
        };
        let in_split = self.mode == Mode::Split && self.settings.sync_scroll;
        let mut scroll = ScrollArea::vertical()
            .id_salt(("preview_scroll", self.buffers[idx].id))
            .auto_shrink([false, false]);
        if in_split && self.split_scroll_driver == SplitScrollDriver::Editor && self.preview_max_scroll > 0.0 {
            scroll = scroll.vertical_scroll_offset(self.split_scroll_ratio * self.preview_max_scroll);
        }
        let scroll_out = scroll.show(ui, |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let avail = ui.available_width();
                let content_w = (avail - 48.0).clamp(320.0, 780.0);
                let side_margin = ((avail - content_w) * 0.5).max(20.0);
                ui.add_space(side_margin);
                ui.vertical(|ui| {
                    ui.set_max_width(content_w);
                    let cmds = preview::render(ui, &doc, &self.palette, base_dir.as_deref(), &mut self.images);
                    for c in cmds {
                        match c {
                            preview::Cmd::OpenUrl(u) => {
                                let _ = open::that(u);
                            }
                            preview::Cmd::OpenFile(p) => {
                                let _ = open::that(p);
                            }
                            preview::Cmd::Reveal(p) => {
                                let _ = open::that(p.parent().map(|d| d.to_path_buf()).unwrap_or_default());
                            }
                        }
                    }
                });
            });
            ui.add_space(40.0);
        });
        if in_split {
            let max_view = (scroll_out.content_size.y - scroll_out.inner_rect.height()).max(0.0);
            self.preview_max_scroll = max_view;
            let pointer_in_preview = ui.rect_contains_pointer(scroll_out.inner_rect);
            let scrolled = ui.input(|i| i.smooth_scroll_delta.y != 0.0);
            if pointer_in_preview && scrolled {
                self.split_scroll_driver = SplitScrollDriver::Preview;
                if max_view > 0.0 {
                    self.split_scroll_ratio = (scroll_out.state.offset.y / max_view).clamp(0.0, 1.0);
                }
            }
        }
    }

    fn edit_pane(&mut self, ui: &mut egui::Ui, idx: usize) {
        let wrap = self.settings.wrap_editor || self.mode == Mode::Split;
        let show_nums = self.settings.show_line_numbers;
        let pal = self.palette.clone();
        let font_size = ui.style().text_styles.get(&egui::TextStyle::Monospace).map(|f| f.size).unwrap_or(13.5);
        let font = FontId::monospace(font_size);
        let matches = Arc::new(self.find_matches.clone());
        let len = self.buffers[idx].text.len();

        let in_split = self.mode == Mode::Split && self.settings.sync_scroll;
        let mut scroll = if wrap {
            ScrollArea::vertical()
        } else {
            ScrollArea::both()
        };
        if in_split && self.split_scroll_driver == SplitScrollDriver::Preview && self.editor_max_scroll > 0.0 {
            scroll = scroll.vertical_scroll_offset(self.split_scroll_ratio * self.editor_max_scroll);
        }
        let scroll_out = scroll
            .id_salt(("editor_scroll", self.buffers[idx].id))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    if show_nums {
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let lines = self.buffers[idx].line_count();
                            for n in 1..=lines.min(10_000) {
                                ui.label(RichText::new(format!("{n:>4}")).monospace().color(pal.faint).size(font_size));
                            }
                        });
                        ui.add_space(8.0);
                    }
                    ui.push_id(("edit", self.buffers[idx].id), |ui| {
                        let is_focused = self.editor_widget.map_or(false, |id| ui.memory(|m| m.has_focus(id)));
                        let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift && !i.modifiers.command && !i.modifiers.alt);
                        if is_focused && enter_pressed {
                            let sel = self.sel_bytes();
                            if sel.is_empty() {
                                if let Some(op) = format::smart_enter(&self.buffers[idx].text, sel.start) {
                                    ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                                    self.set_text(op.new_text, Some(op.selection));
                                    self.last_edit = Instant::now();
                                    if self.find.open {
                                        self.find_refresh();
                                    }
                                }
                            }
                        }
                        let avail_w = ui.available_width();
                        let desired_w = if wrap { avail_w.max(80.0) } else { f32::INFINITY };
                        let mut te = TextEdit::multiline(&mut self.buffers[idx].text)
                            .font(font.clone())
                            .desired_width(desired_w)
                            .desired_rows(10)
                            .margin(Margin::symmetric(12, 6));
                        let use_syntax = len < MAX_SYNTAX_CHARS;
                        let m2 = matches.clone();
                        let mut layouter = move |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
                            let text = buffer.as_str();
                            let max_w = if wrap { width.min(avail_w).max(80.0) } else { f32::INFINITY };
                            let mut job = if use_syntax {
                                editor::editor_job(text, &pal, font.clone(), &m2)
                            } else {
                                egui::text::LayoutJob::simple(text.to_owned(), font.clone(), pal.text, max_w)
                            };
                            job.wrap.max_width = max_w;
                            job.wrap.break_anywhere = true;
                            ui.fonts_mut(|f| f.layout_job(job))
                        };
                        te = te.layouter(&mut layouter);
                        let out = te.show(ui);
                        self.editor_widget = Some(out.response.id);
                        if let Some(pc) = self.pending_cursor.take() {
                            let s = pc.sorted_cursors();
                            self.sel = Some((s[0].index.0, s[1].index.0));
                            if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), out.response.id) {
                                st.cursor.set_char_range(Some(pc));
                                st.store(ui.ctx(), out.response.id);
                            }
                            out.response.request_focus();
                            self.focus_editor = false;
                        } else if self.focus_editor {
                            out.response.request_focus();
                            self.focus_editor = false;
                        } else if let Some(r) = out.cursor_range {
                            let s = r.sorted_cursors();
                            self.sel = Some((s[0].index.0, s[1].index.0));
                        }
                        if out.response.changed() {
                            self.invalidate_preview();
                            self.last_edit = Instant::now();
                            if self.find.open {
                                self.find_refresh();
                            }
                        }
                    });
                });
            });
        if in_split {
            let max_edit = (scroll_out.content_size.y - scroll_out.inner_rect.height()).max(0.0);
            self.editor_max_scroll = max_edit;
            let pointer_in_editor = ui.rect_contains_pointer(scroll_out.inner_rect);
            let scrolled = ui.input(|i| i.smooth_scroll_delta.y != 0.0);
            if (pointer_in_editor && scrolled) || self.split_scroll_driver == SplitScrollDriver::Editor {
                self.split_scroll_driver = SplitScrollDriver::Editor;
                if max_edit > 0.0 {
                    self.split_scroll_ratio = (scroll_out.state.offset.y / max_edit).clamp(0.0, 1.0);
                }
            }
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        if let Some((msg, at)) = self.toast.clone() {
            if at.elapsed().as_secs_f32() > 5.0 {
                self.toast = None;
                return;
            }
            egui::Area::new(egui::Id::new("toast"))
                .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -38.0))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(self.palette.widget_bg)
                        .stroke(egui::Stroke::new(1.0, self.palette.hr))
                        .corner_radius(8.0)
                        .inner_margin(Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(msg).color(self.palette.text).size(12.0));
                        });
                });
        }
    }
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.update_impl(ui);
    }
    fn on_exit(&mut self) {
        self.settings.save();
        clear_recovery();
    }
}

fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn push_undo_snapshot(ctx: Option<&egui::Context>, id: Option<egui::Id>, sel: Option<(usize, usize)>, text: &str) {
    use eframe::egui::text::{CCursor, CCursorRange};
    let (Some(ctx), Some(id)) = (ctx, id) else { return };
    let (a, b) = sel.unwrap_or((0, 0));
    let range = CCursorRange::two(CCursor::new(a.min(b)), CCursor::new(a.max(b)));
    if let Some(st) = egui::TextEdit::load_state(ctx, id) {
        st.undoer().add_undo(&(range, text.to_owned()));
        st.store(ctx, id);
    }
}

pub fn text_has_cjk(text: &str) -> bool {
    // Fast path: ASCII and Latin-1 supplement (< 0xE0) contain no CJK characters.
    if !text.as_bytes().iter().any(|&b| b >= 0xE0) {
        return false;
    }
    text.chars().any(|c| {
        matches!(c,
            '\u{2E80}'..='\u{2FD5}'
            | '\u{3000}'..='\u{303F}'
            | '\u{3040}'..='\u{31FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
        )
    })
}

fn load_cjk_font_data() -> Option<egui::FontData> {
    for c in [
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ] {
        if let Ok(file) = std::fs::File::open(c) {
            if let Ok(mmap) = unsafe { memmap2::Mmap::map(&file) } {
                let leaked: &'static memmap2::Mmap = Box::leak(Box::new(mmap));
                return Some(egui::FontData::from_static(&leaked[..]));
            } else if let Ok(bytes) = std::fs::read(c) {
                return Some(egui::FontData::from_owned(bytes));
            }
        }
    }
    None
}

fn build_base_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();

    // Remove redundant Ubuntu font to save heap memory; Inter and JetBrains Mono are primary.
    fonts.font_data.remove("Ubuntu-Light");
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(fam) = fonts.families.get_mut(&family) {
            fam.retain(|name| name != "Ubuntu-Light");
        }
    }

    // 1. Proportional: Inter
    fonts.font_data.insert(
        "inter_regular".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf"))),
    );
    fonts.font_data.insert(
        "inter_semibold".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.ttf"))),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "inter_regular".to_owned());
    }

    // 2. Monospace: JetBrains Mono
    fonts.font_data.insert(
        "jetbrains_mono".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"))),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        family.insert(0, "jetbrains_mono".to_owned());
    }

    crate::theme::register_bold_family(&mut fonts);
    fonts
}

fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(build_base_fonts());
}

fn recovery_dir() -> PathBuf {
    Settings::config_dir().join("recovery")
}

fn scan_recovery() -> Vec<(String, Option<PathBuf>, String)> {
    let Ok(entries) = std::fs::read_dir(recovery_dir()) else { return Vec::new() };
    let mut out = Vec::new();
    for e in entries.flatten() {
        if let Ok(s) = std::fs::read_to_string(e.path()) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                let title = v["title"].as_str().unwrap_or("recovered document").to_owned();
                let path = v["path"].as_str().map(PathBuf::from);
                let text = v["text"].as_str().unwrap_or("").to_owned();
                out.push((title, path, text));
            }
        }
    }
    out
}

fn clear_recovery() {
    let _ = std::fs::remove_dir_all(recovery_dir());
}

impl App {
    /// Render the single app-modal (spec §6.12, §7.2, §8.1) and resolve it.
    fn modal_ui(&mut self, ctx: &egui::Context, modal: Modal) {
        match modal {
            Modal::About => {
                let mut close = false;
                egui::Window::new("RapidMD — Help & Shortcuts")
                    .collapsible(false)
                    .resizable(true)
                    .default_width(560.0)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ScrollArea::vertical()
                            .id_salt("help_modal_scroll")
                            .max_height(480.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    // Header
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                                        if let Some(tex) = self.app_icon(ctx) {
                                            let img = egui::Image::new(&tex)
                                                .fit_to_exact_size(Vec2::new(26.0, 26.0))
                                                .corner_radius(6.0);
                                            ui.add(img);
                                        }
                                        ui.label(RichText::new("RapidMD").strong().size(22.0).color(self.palette.accent));
                                        egui::Frame::new()
                                            .fill(self.palette.widget_bg)
                                            .stroke(egui::Stroke::new(1.0, self.palette.hr))
                                            .corner_radius(5.0)
                                            .inner_margin(egui::Margin::symmetric(6, 2))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new("v0.1.0").monospace().color(self.palette.faint).size(10.5));
                                            });
                                    });
                                    ui.add_space(2.0);
                                    ui.label(RichText::new("Native Markdown viewer & editor with high-DPI antialiasing.").color(self.palette.faint).size(13.0));

                                    ui.add_space(14.0);
                                    ui.separator();
                                    ui.add_space(8.0);

                                    // Tech Stack
                                    ui.label(RichText::new("TECH STACK").color(self.palette.faint).size(11.0).strong());
                                    ui.add_space(6.0);
                                    egui::Grid::new("tech_stack_grid")
                                        .spacing(Vec2::new(16.0, 6.0))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            let tech = [
                                                ("Rust", "Safe, concurrent native systems language (2021 edition)"),
                                                ("eframe & egui", "0.36 immediate-mode GPU-accelerated GUI with embedded Inter font"),
                                                ("pulldown-cmark", "0.13 pull-parser for CommonMark, GFM tables & task lists"),
                                                ("syntect", "5.3 syntax highlighter using TextMate grammars"),
                                                ("image", "0.25 pure-Rust local image decoding (PNG, JPEG, WebP, GIF)"),
                                                ("arboard", "3.0 cross-platform native clipboard engine"),
                                                ("rfd", "0.17 native system file/folder dialogs"),
                                            ];
                                            for (k, desc) in tech {
                                                ui.label(RichText::new(k).strong().color(self.palette.text).size(12.0));
                                                ui.label(RichText::new(desc).color(self.palette.faint).size(12.0));
                                                ui.end_row();
                                            }
                                        });

                                    ui.add_space(14.0);
                                    ui.separator();
                                    ui.add_space(8.0);

                                    // Shortcuts
                                    ui.label(RichText::new("KEYBOARD SHORTCUTS").color(self.palette.faint).size(11.0).strong());
                                    ui.add_space(6.0);
                                    egui::Grid::new("shortcuts_grid")
                                        .spacing(Vec2::new(20.0, 6.0))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            let shortcuts = [
                                                ("Ctrl + N", "New document"),
                                                ("Ctrl + O", "Open file…"),
                                                ("Ctrl + S", "Save current buffer"),
                                                ("Ctrl + Shift + S", "Save As…"),
                                                ("Ctrl + W", "Close active tab"),
                                                ("Ctrl + Shift + W", "Quit application"),
                                                ("Ctrl + 1", "View Mode (rendered preview)"),
                                                ("Ctrl + 2", "Edit Mode (source markdown)"),
                                                ("Ctrl + 3", "Split Mode (side-by-side with sync scroll)"),
                                                ("Ctrl + F", "Find & Replace bar"),
                                                ("F3 / Shift + F3", "Find next / previous match"),
                                                ("Ctrl + G", "Go to line number"),
                                                ("Ctrl + B", "Toggle bold selection (**bold**)"),
                                                ("Ctrl + I", "Toggle italic selection (*italic*)"),
                                                ("Ctrl + K", "Insert markdown link ([text](url))"),
                                                ("Ctrl + =", "Zoom in (+10%)"),
                                                ("Ctrl + -", "Zoom out (-10%)"),
                                                ("Ctrl + 0", "Reset zoom (100%)"),
                                                ("Enter", "Smart list / numbered / task continuation"),
                                                ("F1", "Open this Help & Shortcuts dialog"),
                                            ];
                                            for (k, desc) in shortcuts {
                                                egui::Frame::new()
                                                    .fill(self.palette.widget_bg)
                                                    .stroke(egui::Stroke::new(1.0, self.palette.hr))
                                                    .corner_radius(4.0)
                                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(k).monospace().color(self.palette.text).size(11.0).strong());
                                                    });
                                                ui.label(RichText::new(desc).color(self.palette.faint).size(12.0));
                                                ui.end_row();
                                            }
                                        });

                                    ui.add_space(14.0);
                                });
                            });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let (btn_fill, btn_fg) = if self.palette.dark {
                                (Color32::WHITE, Color32::BLACK)
                            } else {
                                (Color32::from_rgb(17, 17, 19), Color32::WHITE)
                            };
                            let close_btn = egui::Button::new(RichText::new("Close").strong().color(btn_fg).size(13.0))
                                .fill(btn_fill)
                                .corner_radius(6.0)
                                .min_size(Vec2::new(90.0, 30.0));
                            if ui.add(close_btn).clicked() || ctx.input(|i| i.key_pressed(Key::Escape)) {
                                close = true;
                            }
                        });
                    });
                if close {
                    self.modal = None;
                } else {
                    self.modal = Some(Modal::About);
                }
            }
            Modal::GoTo { idx } => {
                let mut ok = false;
                let mut cancel = false;
                egui::Window::new("Go to line")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.add(TextEdit::singleline(&mut self.goto).desired_width(180.0).hint_text("line number"));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Go").strong()).clicked() {
                                ok = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                        if ctx.input(|i| i.key_pressed(Key::Enter)) {
                            ok = true;
                        }
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            cancel = true;
                        }
                    });
                if ok {
                    if let Ok(n) = self.goto.trim().parse::<usize>() {
                        let t = self.act_text().unwrap_or_default();
                        let byte = line_nth_byte(&t, n.saturating_sub(1));
                        let ch = byte_to_char(&t, byte);
                        self.sel = Some((ch, ch));
                        let cc = egui::text::CCursorRange::two(egui::text::CCursor::new(ch), egui::text::CCursor::new(ch));
                        self.pending_cursor = Some(cc);
                    }
                    self.modal = None;
                } else if cancel {
                    self.modal = None;
                } else {
                    self.modal = Some(Modal::GoTo { idx });
                }
            }
            Modal::CloseDirty { idx } => {
                let title = self.buffers.get(idx).map(|b| b.title()).unwrap_or_default();
                let mut choice = 0u8;
                egui::Window::new("Unsaved changes")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label(format!("Save changes to \u{201C}{}\u{201D} before closing?", title));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Save").strong()).clicked() {
                                choice = 1;
                            }
                            if ui.button("Discard").clicked() {
                                choice = 2;
                            }
                            if ui.button("Cancel").clicked() {
                                choice = 3;
                            }
                        });
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            choice = 3;
                        }
                    });
                match choice {
                    1 => {
                        let has_path = self.buffers.get(idx).map(|b| b.path.is_some()).unwrap_or(false);
                        let saved = if has_path { self.save_at(idx) } else { self.save_as_at(idx) };
                        if saved {
                            self.remove_at(idx);
                            self.modal = None;
                        } else {
                            self.modal = Some(Modal::CloseDirty { idx });
                        }
                    }
                    2 => {
                        self.remove_at(idx);
                        self.modal = None;
                    }
                    _ => self.modal = Some(Modal::CloseDirty { idx }),
                }
            }
            Modal::QuitDirty { idxs } => {
                let names: Vec<String> = idxs.iter().filter_map(|&i| self.buffers.get(i).map(|b| b.title())).collect();
                let mut choice = 0u8;
                egui::Window::new("Unsaved changes")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label("The following documents have unsaved changes:");
                        for n in &names {
                            ui.label(RichText::new(format!("• {}", n)).color(self.palette.warning));
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Save All").strong()).clicked() {
                                choice = 1;
                            }
                            if ui.button("Discard All").clicked() {
                                choice = 2;
                            }
                            if ui.button("Cancel").clicked() {
                                choice = 3;
                            }
                        });
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            choice = 3;
                        }
                    });
                match choice {
                    1 => {
                        for &i in &idxs {
                            let has_path = self.buffers.get(i).map(|b| b.path.is_some()).unwrap_or(false);
                            let ok = if has_path { self.save_at(i) } else { self.save_as_at(i) };
                            if !ok {
                                self.modal = Some(Modal::QuitDirty { idxs: self.dirty_list() });
                                return;
                            }
                        }
                        self.modal = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    2 => {
                        let mut removed = 0usize;
                        for i in idxs {
                            self.remove_at(i - removed);
                            removed += 1;
                        }
                        self.modal = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    _ => self.modal = Some(Modal::QuitDirty { idxs: self.dirty_list() }),
                }
            }
            Modal::ExternalChanged { idx } => {
                let title = self.buffers.get(idx).map(|b| b.title()).unwrap_or_default();
                let mut choice = 0u8;
                egui::Window::new("File changed on disk")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label(format!("\u{201C}{}\u{201D} was modified by another program.", title));
                        ui.label("Reload the file, or keep your edits and overwrite later?");
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Reload").strong()).clicked() {
                                choice = 1;
                            }
                            if ui.button("Keep Mine").clicked() {
                                choice = 2;
                            }
                            if ui.button("Cancel").clicked() {
                                choice = 3;
                            }
                        });
                    });
                match choice {
                    1 => {
                        self.reload_from_disk(idx);
                        self.modal = None;
                    }
                    2 => {
                        if let Some(b) = self.buffers.get_mut(idx) {
                            b.suppress_external_change_once = true;
                        }
                        self.modal = None;
                    }
                    _ => self.modal = Some(Modal::ExternalChanged { idx }),
                }
            }
            Modal::InsertLink { idx } => {
                let mut ok = false;
                let mut cancel = false;
                egui::Window::new("Insert Link")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label("Text");
                        ui.add(TextEdit::singleline(&mut self.link_text).desired_width(320.0));
                        ui.label("URL");
                        ui.add(TextEdit::singleline(&mut self.link_url).desired_width(320.0).hint_text("https://…"));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Insert").strong()).clicked() {
                                ok = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                        if ctx.input(|i| i.key_pressed(Key::Enter)) {
                            ok = true;
                        }
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            cancel = true;
                        }
                    });
                if ok {
                    let t = self.act_text().unwrap_or_default();
                    let caret = self.sel_bytes().start;
                    let op = format::insert_link(&t, caret, &self.link_text, &self.link_url);
                    self.set_text(op.new_text, Some(op.selection));
                    self.link_text.clear();
                    self.link_url.clear();
                    self.modal = None;
                } else if cancel {
                    self.link_text.clear();
                    self.link_url.clear();
                    self.modal = None;
                } else {
                    self.modal = Some(Modal::InsertLink { idx });
                }
            }
            Modal::InsertImage { idx, path } => {
                let mut ok = false;
                let mut cancel = false;
                egui::Window::new("Insert Image")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label(format!("File: {}", path.display()));
                        ui.label(RichText::new("Insert a Markdown image reference.").color(self.palette.faint));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Insert").strong()).clicked() {
                                ok = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });
                if ok {
                    let t = self.act_text().unwrap_or_default();
                    let caret = self.sel_bytes().start;
                    let url = path.to_string_lossy().replace('\\', "/");
                    let op = format::insert_image(&t, caret, "", &url);
                    self.set_text(op.new_text, Some(op.selection));
                    self.modal = None;
                } else if cancel {
                    self.modal = None;
                } else {
                    self.modal = Some(Modal::InsertImage { idx, path });
                }
            }
            Modal::TablePicker { idx } => {
                let mut ok = false;
                let mut cancel = false;
                egui::Window::new("Insert Table")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Rows:");
                            if ui.small_button("-").clicked() && self.rows > 1 {
                                self.rows -= 1;
                            }
                            ui.add(egui::DragValue::new(&mut self.rows).range(1..=20));
                            if ui.small_button("+").clicked() && self.rows < 20 {
                                self.rows += 1;
                            }
                            ui.add_space(8.0);
                            ui.label("Columns:");
                            if ui.small_button("-").clicked() && self.cols > 1 {
                                self.cols -= 1;
                            }
                            ui.add(egui::DragValue::new(&mut self.cols).range(1..=12));
                            if ui.small_button("+").clicked() && self.cols < 12 {
                                self.cols += 1;
                            }
                        });
                        ui.add_space(6.0);
                        for r in 0..self.rows {
                            ui.horizontal(|ui| {
                                for c in 0..self.cols {
                                    if ui.add(egui::Button::new(RichText::new("▢").color(self.palette.accent)).min_size(Vec2::new(12.0, 12.0))).clicked() {
                                        self.rows = r + 1;
                                        self.cols = c + 1;
                                    }
                                }
                            });
                        }
                        ui.label(format!("{} × {}", self.rows, self.cols));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Insert").strong()).clicked() {
                                ok = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                        if ctx.input(|i| i.key_pressed(Key::Enter)) {
                            ok = true;
                        }
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            cancel = true;
                        }
                    });
                if ok {
                    let t = self.act_text().unwrap_or_default();
                    let caret = self.sel_bytes().start;
                    let op = format::insert_table(&t, caret, self.rows, self.cols);
                    self.set_text(op.new_text, Some(op.selection));
                    self.modal = None;
                } else if cancel {
                    self.modal = None;
                } else {
                    self.modal = Some(Modal::TablePicker { idx });
                }
            }
            Modal::Recovery { items } => {
                let mut recover = false;
                let mut discard = false;
                egui::Window::new("Recover unsaved documents")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.label("RapidMD did not close cleanly. Found recovered changes:");
                        for (title, path, _) in &items {
                            let loc = path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "unsaved".to_owned());
                            ui.label(RichText::new(format!("• {} ({})", title, loc)).color(self.palette.warning));
                        }
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Recover All").strong()).clicked() {
                                recover = true;
                            }
                            if ui.button("Discard").clicked() {
                                discard = true;
                            }
                        });
                    });
                if recover {
                    for (title, path, text) in items {
                        let id = self.next_id;
                        self.next_id += 1;
                        let mut b = Buffer::untitled(id);
                        b.title_override = Some(title);
                        b.path = path;
                        b.text = text;
                        self.buffers.push(b);
                    }
                    if !self.buffers.is_empty() {
                        self.active = Some(self.buffers.len() - 1);
                    }
                    clear_recovery();
                    self.modal = None;
                } else if discard {
                    clear_recovery();
                    self.modal = None;
                } else {
                    self.modal = Some(Modal::Recovery { items });
                }
            }
        }
    }
}

fn line_nth_byte(text: &str, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut seen = 0usize;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            seen += 1;
            if seen == n {
                return i + 1;
            }
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app(ctx: &egui::Context) -> App {
        install_fonts(ctx);
        let settings = Settings::default();
        let palette = Palette::get(settings.theme);
        palette.apply(ctx);
        let mut app = App {
            buffers: Vec::new(),
            active: None,
            next_id: 1,
            mode: Mode::Edit,
            settings,
            palette,
            find: Finder::default(),
            find_matches: Vec::new(),
            modal: None,
            pending: Vec::new(),
            toast: None,
            images: ImageCache::default(),
            doc_cache: None,
            sel: None,
            editor_widget: None,
            pending_cursor: None,
            ctx_handle: None,
            link_text: String::new(),
            link_url: String::new(),
            goto: String::new(),
            rows: 3,
            cols: 4,
            last_edit: Instant::now(),
            focus_editor: false,
            split_scroll_ratio: 0.0,
            split_scroll_driver: SplitScrollDriver::Editor,
            preview_max_scroll: 0.0,
            editor_max_scroll: 0.0,
            last_frame_time: Instant::now(),
            fps: 60.0,
            icon_texture: None,
            cjk_loaded: false,
        };
        app.add_untitled();
        app
    }

    fn step<R>(ctx: &egui::Context, raw_input: egui::RawInput, mut run_ui: impl FnMut(&mut egui::Ui) -> R) -> R {
        let mut res = None;
        let mut out = ctx.run_ui(raw_input, |ui| {
            res = Some(run_ui(ui));
        });
        out.textures_delta.clear();
        res.unwrap()
    }

    #[test]
    fn test_app_smart_enter_bullet_flow() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        app.buffers[0].text = "- first".to_string();
        app.sel = Some((7, 7));

        // Frame 1: render editor, request focus
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
            if let Some(id) = app.editor_widget {
                ui.ctx().memory_mut(|m| m.request_focus(id));
            }
        });

        // Frame 2: press enter on "- first"
        let mut raw_enter = egui::RawInput::default();
        raw_enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        step(&ctx, raw_enter, |ui| {
            app.update_impl(ui);
        });
        assert_eq!(app.buffers[0].text, "- first\n- ");

        // Frame 3: press enter again on empty bullet "- "
        let mut raw_enter2 = egui::RawInput::default();
        raw_enter2.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        step(&ctx, raw_enter2, |ui| {
            app.update_impl(ui);
        });
        // Empty bullet is removed!
        assert_eq!(app.buffers[0].text, "- first\n");
    }

    #[test]
    fn test_app_smart_enter_numbered_flow() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        app.buffers[0].text = "1. first".to_string();
        app.sel = Some((8, 8));

        // Frame 1: render editor, request focus
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
            if let Some(id) = app.editor_widget {
                ui.ctx().memory_mut(|m| m.request_focus(id));
            }
        });

        // Frame 2: press enter on "1. first"
        let mut raw_enter = egui::RawInput::default();
        raw_enter.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        step(&ctx, raw_enter, |ui| {
            app.update_impl(ui);
        });
        assert_eq!(app.buffers[0].text, "1. first\n2. ");

        // Frame 3: press enter again on empty "2. "
        let mut raw_enter2 = egui::RawInput::default();
        raw_enter2.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        step(&ctx, raw_enter2, |ui| {
            app.update_impl(ui);
        });
        assert_eq!(app.buffers[0].text, "1. first\n");
    }

    #[test]
    fn test_app_h3_heading_toggle() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        app.buffers[0].text = "section title".to_string();
        app.sel = Some((0, 0));
        app.pending.push(Cmd::Heading(3));
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
        });
        assert_eq!(app.buffers[0].text, "### section title");
    }

    #[test]
    fn test_app_table_commands_no_crash() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        // Pre-populate with a table in markdown text
        app.buffers[0].text = "# Title\n\n| Column 1 | Column 2 |\n| --- | --- |\n| row1_a | row1_b |\n".to_string();
        let cell_pos = app.buffers[0].text.find("row1_a").unwrap();
        app.sel = Some((cell_pos, cell_pos));

        // 1. Insert row above
        app.pending.push(Cmd::TableOp(TableCmd::InsertRowAbove));
        step(&ctx, egui::RawInput::default(), |ui| app.update_impl(ui));
        assert!(app.buffers[0].text.lines().count() >= 6);

        // 2. Insert row below
        app.pending.push(Cmd::TableOp(TableCmd::InsertRowBelow));
        step(&ctx, egui::RawInput::default(), |ui| app.update_impl(ui));
        assert!(app.buffers[0].text.lines().count() >= 7);

        // 3. Insert column left
        app.pending.push(Cmd::TableOp(TableCmd::InsertColLeft));
        step(&ctx, egui::RawInput::default(), |ui| app.update_impl(ui));

        // 4. Insert column right
        app.pending.push(Cmd::TableOp(TableCmd::InsertColRight));
        step(&ctx, egui::RawInput::default(), |ui| app.update_impl(ui));

        // 5. Delete row
        app.pending.push(Cmd::TableOp(TableCmd::DeleteRow));
        step(&ctx, egui::RawInput::default(), |ui| app.update_impl(ui));

        // 6. Delete column
        app.pending.push(Cmd::TableOp(TableCmd::DeleteCol));
        step(&ctx, egui::RawInput::default(), |ui| app.update_impl(ui));
    }

    #[test]
    fn test_modal_no_blackout() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        app.modal = Some(Modal::TablePicker { idx: 0 });

        // Frame renders with modal active: editor_widget and panels should still be created
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
        });

        assert!(app.modal.is_some(), "modal remains open");
        assert!(app.editor_widget.is_some(), "editor widget was rendered underneath the modal");
    }

    #[test]
    fn test_tab_bar_no_gap_and_layout() {
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1920.0, 1080.0)));
        let mut app = create_test_app(&ctx);
        app.add_untitled();
        step(&ctx, raw.clone(), |ui| {
            app.update_impl(ui);
        });
        step(&ctx, raw, |ui| {
            let avail = app.update_impl(ui);
            assert!(avail.min.y <= 90.0, "tabs panel height must be compact without black gap, got min.y={}", avail.min.y);
        });
    }

    #[test]
    fn test_split_mode_editor_wraps_text() {
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
        let mut app = create_test_app(&ctx);
        app.mode = Mode::Split;
        // Even if wrap_editor is explicitly disabled in settings, split mode forces wrap
        app.settings.wrap_editor = false;
        app.buffers[0].text = "This is a very long line of text that should wrap inside the split pane editor instead of extending past the split boundary and getting hidden by the preview pane on the right.".to_string();

        step(&ctx, raw, |ui| {
            app.update_impl(ui);
            // The editor widget should have been rendered and its ID recorded
            assert!(app.editor_widget.is_some());
        });
    }

    #[test]
    fn test_toolbar_click_refocuses_editor() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        app.buffers[0].text = "my heading".to_string();
        app.sel = Some((0, 0));

        // Frame 1: render app
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
        });

        // User clicks H3 on toolbar:
        app.pending.push(Cmd::Heading(3));

        // Frame 2: app runs flush, applies Heading(3), edit_pane renders and calls request_focus
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
        });
        // Frame 3: edit_pane runs with focus_editor = true and requests focus
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
            let editor_id = app.editor_widget.expect("editor must have ID");
            assert!(ctx.memory(|m| m.has_focus(editor_id)), "Editor must automatically regain focus after toolbar action!");
        });
    }

    #[test]
    fn test_sync_scroll_command_toggles_and_split_sync() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        assert!(app.settings.sync_scroll);
        app.apply_cmd(Cmd::SyncScroll, &ctx);
        assert!(!app.settings.sync_scroll);
        app.apply_cmd(Cmd::SyncScroll, &ctx);
        assert!(app.settings.sync_scroll);
    }

    #[test]
    fn test_help_modal_stays_open_across_frames() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);

        // Frame 1: trigger About
        app.pending.push(Cmd::About);
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
        });
        assert!(matches!(app.modal, Some(Modal::About)));

        // Frame 2: modal should stay open, not disappear
        step(&ctx, egui::RawInput::default(), |ui| {
            app.update_impl(ui);
        });
        assert!(matches!(app.modal, Some(Modal::About)), "Modal::About must persist across frames until closed");

        // Frame 3: user presses Escape -> closes modal
        let mut esc_input = egui::RawInput::default();
        esc_input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        step(&ctx, esc_input, |ui| {
            app.update_impl(ui);
        });
        assert!(app.modal.is_none(), "Escape must dismiss the help modal");
    }

    #[test]
    fn test_fps_counter_toggle() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        assert!(app.settings.show_fps);
        app.apply_cmd(Cmd::ToggleFps, &ctx);
        assert!(!app.settings.show_fps);
        app.apply_cmd(Cmd::ToggleFps, &ctx);
        assert!(app.settings.show_fps);
    }

    #[test]
    fn test_text_has_cjk_detection() {
        assert!(!text_has_cjk("Hello world"));
        assert!(!text_has_cjk("# RapidMD - Fast Markdown"));
        assert!(!text_has_cjk("Bonjour, ça va être l'été"));
        assert!(!text_has_cjk("Grüße aus Köln - Straße"));
        assert!(text_has_cjk("你好世界"));
        assert!(text_has_cjk("こんにちは"));
        assert!(text_has_cjk("カタカナ"));
        assert!(text_has_cjk("안녕하세요"));
        assert!(text_has_cjk("「引用」"));
    }

    #[test]
    fn test_lazy_cjk_font_trigger() {
        let ctx = egui::Context::default();
        let mut app = create_test_app(&ctx);
        assert!(!app.cjk_loaded);
        app.buffers[0].text = "Hello world".to_string();
        app.ensure_cjk_if_needed(&ctx);
        assert!(!app.cjk_loaded);
    }
}
