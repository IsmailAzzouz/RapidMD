# Core Application & Lifecycle Modules (`core_lifecycle`)

This document provides detailed technical documentation for the core application and lifecycle modules of RapidMD (`src/main.rs`, `src/lib.rs`, `src/app.rs`, `src/config.rs`, and `src/io.rs`).

---

## 1. Module Structure & Crate Root (`lib.rs`)

`src/lib.rs` acts as the library root for `rustdown_viewer`, declaring all public submodules that comprise the application:

```rust
pub mod app;
pub mod buffer;
pub mod config;
pub mod dialogs;
pub mod editor;
pub mod find;
pub mod format;
pub mod highlight;
pub mod html;
pub mod io;
pub mod md;
pub mod preview;
pub mod text_utils;
pub mod theme;
```

Each submodule encapsulates a distinct domain:
- **`app`**: Main application shell, UI rendering loop, state management, and command handling.
- **`config`**: Persistent settings and recent files management.
- **`io`**: Encoding-aware file reading and atomic saving.
- **`buffer`**: Document buffer abstractions and modification tracking.
- **`editor` / `preview` / `md`**: Markdown parsing, AST rendering, syntax highlighting, and split-screen preview.

---

## 2. Application Startup (`main.rs`)

`src/main.rs` serves as the thin native binary entry point. It enforces safety invariants, configures window compilation settings, embeds application icons, and starts the `eframe`/`egui` GUI runtime.

### Key Aspects:
1. **Safety & Lints**:
   - `#![forbid(unsafe_code)]`: Completely prohibits unsafe blocks across the binary crate.
   - `#![deny(clippy::all)]`: Enforces strict Clippy lint checks.
   - `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`: Suppresses the console window in release builds on Windows.

2. **Icon Embedding & Resizing (`load_app_icon`)**:
   - Uses `include_bytes!("../assets/Icon/RMD.png")` to embed the application icon at compile time.
   - Decodes the image from memory via the `image` crate, resizes it to 256x256 using Lanczos3 filtering, converts it to RGBA8 format, and packages it into `egui::IconData`.

3. **Viewport & Native Options**:
   - Configures `egui::ViewportBuilder` with title `"RapidMD"`, initial inner size `[1280.0, 840.0]`, and minimum inner size `[720.0, 480.0]`.
   - Attaches the loaded icon if available.
   - Wraps options in `eframe::NativeOptions`.

4. **Runtime Launch**:
   - Calls `eframe::run_native("RapidMD", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))` to bootstrap the event loop and initialize the primary `App` state.

---

## 3. State Management & Event Loop (`app.rs`)

`src/app.rs` contains the core application shell, managing document tabs (`Buffer`), UI modes, panels, modal dialogs, themes, shortcuts, and rendering routines.

### Key Structures:
- **`Mode`**: Represents the current display/editing mode:
  - `View`: Pure Markdown rendered preview mode.
  - `Edit`: Pure source code editor mode.
  - `Split`: Side-by-side synchronized split view.
- **`Cmd`**: Enumeration of all application commands (file operations, formatting actions, zoom levels, view modes, search/replace, theme selection).
- **`App`**: The central state container implementing `eframe::App`. It tracks:
  - `buffers`: Collection of open document buffers.
  - `active`: Index of the currently active tab.
  - `mode`: Current editor/viewer mode.
  - `settings`: Persistent configuration (`Settings`).
  - `palette`: Current active color theme palette.
  - `find`: Search and replacement engine state (`Finder`).
  - `modal`: Active modal dialog overlay (`Option<Modal>`).
  - `images`: Image cache for rendered markdown images.
  - `doc_cache`: Cached parsed Markdown AST document.
  - Viewport scroll ratios, FPS tracking, toast notifications, and cursor states.

### Event Loop & Rendering:
- Driven by `eframe::App::update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame)`.
- Handles keyboard shortcuts (e.g., Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+F, Ctrl+Tab), renders top menu bars, tab bars, main workspace (editor and/or preview), status bars, and modal dialogs.
- Manages synchronized scrolling in split mode between editor and preview panes.

---

## 4. Configuration Loading & Persistence (`config.rs`)

`src/config.rs` manages persistent user settings and recent files history, adhering to specification requirements with atomic JSON serialization.

### The `Settings` Struct:
```rust
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub theme: ThemeMode,
    pub zoom: f32,
    pub wrap_editor: bool,
    pub show_line_numbers: bool,
    pub recent: Vec<String>,
    #[serde(default = "default_true")]
    pub sync_scroll: bool,
    #[serde(default = "default_true")]
    pub show_fps: bool,
}
```

### Key Operations:
- **Directory Resolution (`config_dir`)**: Uses the `dirs` crate to locate the platform-specific configuration directory (`dirs::config_dir()`), appending `RustDownViewer`.
- **Loading (`Settings::load`)**: Reads `config.json` from the configuration directory, deserializing JSON into `Settings`. Falls back to `Settings::default()` on failure or absence.
- **Saving (`Settings::save`)**: Ensures the config directory exists (`fs::create_dir_all`), serializes settings to pretty-printed JSON, and writes atomically.
- **Recent Files (`push_recent`)**: Maintains a deduplicated queue of up to 10 recently opened file paths.

---

## 5. File I/O Operations (`io.rs`)

`src/io.rs` provides robust, encoding-aware file reading and atomic saving mechanisms (specs F80–F83).

### File Reading (`read_file` & `decode_bytes`):
1. **Reading Bytes**: Reads file contents via `fs::read`, returning `ReadError::NotFound` or `ReadError::Io`.
2. **Metadata & Read-Only Probing (`probe_read_only`)**:
   - Extracts `DiskStamp` (modification time, size, etc.).
   - On Windows, checks `FILE_ATTRIBUTE_READONLY` in file metadata.
   - On Unix (and as a fallback on Windows), tests write access via `OpenOptions::new().write(true).open(path)`.
3. **Encoding Detection (`decode_bytes`)**:
   - **UTF-8 with BOM**: Detects `[0xEF, 0xBB, 0xBF]` prefix.
   - **UTF-16 LE / BE**: Detects `[0xFF, 0xFE]` (Little Endian) or `[0xFE, 0xFF]` (Big Endian) BOM markers and decodes code units.
   - **Plain UTF-8**: Validates standard UTF-8.
   - **Lossy Fallback**: Falls back gracefully to `String::from_utf8_lossy` for non-UTF-8 encodings without failing.

### Atomic Saving (`atomic_save`):
To prevent data corruption or partial writes during disk persistence:
1. **Read-Only Safeguard**: Checks if the target file exists and is marked read-only, returning `SaveError::ReadOnly` to avoid clobbering.
2. **Temporary File Creation**: Constructs a temporary file name in the same directory as the target (e.g., `.{file_name}.rdt-{pid}`).
3. **Write & Sync**: Writes bytes to the temp file and calls `f.sync_all()` to ensure physical disk flushing.
4. **Atomic Rename**: Drops the file handle and executes `fs::rename(&tmp_path, path)`, atomically replacing the target file.
5. **Cleanup**: In case of any I/O error during the write or sync phase, the temporary file is automatically removed.
