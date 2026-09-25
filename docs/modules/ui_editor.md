# UI & Editor Modules Technical Documentation

This document provides comprehensive technical documentation for the core UI and editor components of RustDownViewer:
1. [`buffer.rs`](#1-document-buffer-model-buffer-rs) — Document buffer model and disk synchronization.
2. [`editor.rs`](#2-editor-rendering--text-layout-editor-rs) — Editor view state, line/column tracking, and syntax-colored layout jobs.
3. [`preview.rs`](#3-markdown-preview--syntax-highlighting-preview-rs) — Markdown document rendering, image caching, and syntect code highlighting.
4. [`dialogs.rs`](#4-app-modal-dialogs-dialogs-rs) — Single-modal dialog state machine and destructive guard modals.
5. [`find.rs`](#5-find--replace-logic-find-rs) — Search, match extraction, and atomic replacement logic.
6. [`theme.rs`](#6-visual-theming--palettes-theme-rs) — System/Light/Dark theme modes, color palettes, and egui visual adaptation.

---

## 1. Document Buffer Model (`buffer.rs`)

The `buffer.rs` module implements the core document buffer model (spec §3.1–§3.5) adhering to the principle: **one buffer = one file = one truth**.

### Key Data Structures

- **`Eol`**: Represents the line-ending convention of a file (`Lf` or `Crlf`).
  - `Eol::detect(raw: &str) -> Eol`: Scans raw file content and picks the dominant line ending (`\r\n` vs `\n`).
  - `Eol::platform_default() -> Eol`: Returns `Crlf` on Windows and `Lf` on POSIX systems.
  - `Eol::apply(&self, internal: &str) -> String`: Converts internal LF-normalized text back to the file's original EOL format upon saving.

- **`DiskStamp`**: A lightweight metadata snapshot used for external-change detection (spec §8.4).
  - Fields: `len: u64`, `modified_millis: Option<u128>`.
  - `DiskStamp::from_meta(meta: &std::fs::Metadata) -> Self`: Extracts file length and modification timestamp in milliseconds since UNIX epoch.
  - `DiskStamp::changed_vs(&self, other: &DiskStamp) -> bool`: Compares file size and modification time to detect external modifications.

- **`Encoding`**: Provenance of the loaded text (`Utf8`, `Utf8Bom`, `Utf16Le`, `Utf16Be`, `Lossy`).
  - `Encoding::label(&self) -> &'static str`: Returns human-readable encoding labels (e.g., `"UTF-8"`, `"repaired"` for lossy fallback).

- **`Buffer`**: Represents an open document.
  - Fields: `id: u64`, `path: Option<PathBuf>` (`None` for untitled buffers), `text: String` (internally normalized to LF), `saved_text: String` (snapshot at last save/load boundary for dirty checking), `eol: Eol`, `encoding: Encoding`, `read_only: bool`, `missing_on_disk: bool`, `disk_stamp: Option<DiskStamp>`, `suppress_external_change_once: bool`, `title_override: Option<String>`.

### Core Buffer Methods

- `Buffer::untitled(id: u64) -> Self`: Creates a new empty untitled buffer with platform default EOL.
- `Buffer::from_disk(...) -> Self`: Loads a buffer from disk, detects EOL, normalizes internal text to `\n`, and records disk stamp and saved text.
- `Buffer::title(&self) -> String`: Resolves file name or falls back to `Untitled-{id}`.
- `Buffer::is_dirty(&self) -> bool`: Returns `true` if current `text` differs from `saved_text`.
- `Buffer::line_count(&self) -> usize`: Counts lines by filtering `\n` bytes (`+ 1`).
- `Buffer::mark_saved(&mut self)`: Updates `saved_text`, clears external change suppression, and refreshes `disk_stamp`.

---

## 2. Editor Rendering & Text Layout (`editor.rs`)

The `editor.rs` module provides rendering helpers for the markdown source editor, combining syntax tokenization (`highlight::tk`) with find-match highlights (spec F42/F49) and caret metrics.

### Key Functions & Layout Logic

- **`editor_job(text: &str, pal: &Palette, font: FontId, matches: &[Range<usize>]) -> LayoutJob`**:
  - Tokenizes the editor text using `tokenize(text)`.
  - Collects segment boundaries from token start/ends and find-match start/ends, sorting and deduplicating them.
  - Maps syntax token kinds (`Tk::Heading`, `Tk::Strong`, `Tk::Emphasis`, `Tk::Code`, etc.) to palette colors (`color_of`).
  - Overrides background colors with `pal.match_bg` for active search/replace matches.
  - Applies bold/italic font styling based on markdown formatting tokens.
  - Sets `job.break_on_newline = true` for correct editor line wrapping.

- **`line_col(text: &str, byte: usize) -> (usize, usize)`**:
  - Computes 1-based line and column numbers for byte offsets, powering the status bar cursor position display.

- **`find_layouter(...)`**:
  - A convenience wrapper returning a closure for `egui::TextEdit` that wraps `editor_job` with an `Arc<Vec<Range<usize>>>` match collection.

---

## 3. Markdown Preview & Syntax Highlighting (`preview.rs`)

The `preview.rs` module renders parsed AST (`crate::md::Doc`) into egui UI components (spec §6.7, F20–F28).

### Architectural Design

- **Interactive Commands (`Cmd`)**: Emits actions like `OpenUrl(String)`, `OpenFile(PathBuf)`, `Reveal(PathBuf)`, and `Toast(String)`.
- **Image Cache (`ImageCache`)**: Decodes local images once using the `image` crate, clamps dimensions (`MAX = 4096` pixels via Lanczos3 resampling), and caches egui `TextureHandle`s.
- **Syntect Code Highlighting**: Uses `syntect` library with default syntaxes and themes (`base16-ocean.dark` for dark mode, `InspiredGitHub` for light mode) to syntax-highlight fenced code blocks.
- **Inline Formatting & Links**: `spans_to_job` converts inline `Span`s into egui `LayoutJob`s while recording character offset ranges and URLs (`LinkHits`) for click hit-testing.
- **Container Frames**: Uses egui `Frame` containers with tinted backgrounds (`pal.code_bg`, `pal.quote_bg`) and left accent bars (`pal.quote_bar`) for code blocks, tables, and blockquotes.

---

## 4. App-Modal Dialogs (`dialogs.rs`)

The `dialogs.rs` module defines the application modal dialog state machine (spec §6.12, §7.2, §8.1).

### Modal State Machine

RustDownViewer enforces a strict single-modal model: **exactly one modal exists at a time as an enum** (not a stack). While a modal is active, the rest of the application is non-interactive.

- **`Modal` Enum Variants**:
  - `CloseDirty { idx }`: Prompts to Save / Discard / Cancel for a single closing tab.
  - `QuitDirty { idxs }`: Prompts for saving multiple dirty buffers on app quit.
  - `ExternalChanged { idx }`: Handles files modified externally on disk (Reload / Keep / Cancel).
  - `InsertLink { idx }`: Interactive link insertion dialog (label + URL).
  - `InsertImage { idx, path }`: Image reference insertion dialog.
  - `TablePicker { idx }`: Grid picker (rows × columns) for inserting markdown tables.
  - `Recovery { items }`: Crash-recovery session restore dialog (spec F68/F91).
  - `GoTo { idx }`: Line number jump dialog.
  - `About`: Application info dialog.

- **`Modal::is_guard(&self) -> bool`**: Returns `true` for destructive guard modals (`CloseDirty`, `QuitDirty`, `ExternalChanged`).
- **`TableDraft`**: Helper struct tracking table dimensions (`rows`, `cols`).

---

## 5. Find & Replace Logic (`find.rs`)

The `find.rs` module implements search and replace state and matching algorithms (spec §6.16, F49).

### `Finder` Struct & Methods

- **Fields**: `query: String`, `replace: String`, `case_sensitive: bool`, `regex: bool`, `open: bool`.
- **`matches(&self, text: &str) -> Vec<Range<usize>>`**:
  - Returns byte offset ranges of all matches.
  - Handles case sensitivity by lowercasing haystacks and needles when `case_sensitive` is false.
- **`replace_at(text: &str, m: &Range<usize>, replacement: &str) -> String`**:
  - Splices the replacement string at the specified match range.
- **`replace_all(&self, text: &str) -> (String, usize)`**:
  - Replaces all occurrences across the text as a single atomic operation, returning the modified text and total match count.

---

## 6. Visual Theming & Palettes (`theme.rs`)

The `theme.rs` module manages visual theming (spec F29) across Light, Dark, and System modes.

### Theme Modes & Palettes

- **`ThemeMode`**: Enum (`System`, `Light`, `Dark`). `os_is_dark()` queries system preference via `dark_light`.
- **`Palette`**: Comprehensive color definitions containing background colors (`window_bg`, `panel_bg`, `widget_bg`), text/accent colors (`text`, `faint`, `accent`, `accent_soft`, `link`), UI feedback colors (`danger`, `warning`, `success`), and **markdown editorial token colors** (`md_heading`, `md_em`, `md_strong`, `md_link`, `md_url`, `md_quote`, `md_list`, `md_hr`, `md_html`, `md_fence`, `md_task`, `match_bg`).
- **Theme Definitions**:
  - `Palette::dark()`: Tesla Minimalist Night / OLED Monochrome palette with deep OLED canvas (`#0c0d0f`) and crisp off-white text.
  - `Palette::light()`: Tesla / Notion Day Clean Slate palette with clean white panels and charcoal text.
- **`Palette::apply(&self, ctx: &egui::Context)`**:
  - Configures egui `Visuals` (panel fill, extreme background, selection colors, hyperlink color, text color).
  - Applies Tesla-style sleek rounded corner radii (`7` for pills/menus, `10` for windows/cards) and window/widget strokes.

---

## Summary & Architectural Compliance

These six modules form a cohesive, decoupled UI and editor subsystem:
- **`buffer.rs`** maintains authoritative text truth and disk state.
- **`editor.rs`** and **`preview.rs`** provide dual-pane editing and rendering views with robust syntax highlighting.
- **`dialogs.rs`** and **`find.rs`** manage modal workflows and search operations cleanly.
- **`theme.rs`** ensures a polished, professional aesthetic across light and dark modes.
