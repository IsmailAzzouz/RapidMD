# UI Redesign Specification: Tesla & Notion Minimalist System

## 1. Executive Summary & Design Vision

This document details the architectural plan to redesign **RustDownViewer** from its current utility-style appearance into a sleek, monochrome, high-precision interface inspired by **Tesla's in-vehicle touch UI** and **Notion's minimalist reading/writing workspace**.

### Core Tenets
- **Monochrome Elegance**: The visual foundation is built on deep blacks, graphite charcoals, crisp whites, and neutral grays. Garish rainbow colors are eliminated in favor of high-contrast editorial typography and subtle tonal layering.
- **Sleek Geometry & Radii**: Generous, uniform rounded corners (6px–12px), pill-shaped segmented controls, and hairline translucent borders replace sharp, utilitarian edges.
- **Modern Typography & Antialiasing**: Replace egui's default basic font stack with modern neo-grotesque sans-serifs (`Segoe UI Variable`, `SF Pro`, `Inter`, `Roboto`) and refined monospaced typefaces (`Cascadia Code`, `SF Mono`, `JetBrains Mono`), coupled with active tessellation feathering for razor-sharp antialiasing.
- **Zero Business Logic Disruption**: Strict preservation of all buffer state lifecycles, markdown parsing ASTs, IO operations, recovery mechanisms, and editor shortcuts. All 65 existing tests must continue to pass without modification.

---

## 2. Color Palette & Token Model

### 2.1 Dark Palette ("Tesla Night / OLED Minimalist")
Designed for high legibility, reduced eye strain, and deep visual contrast without noise.

| Token | Current Value | Tesla/Notion Target | Visual Intent |
| :--- | :--- | :--- | :--- |
| `window_bg` | `#13151A` | `#0D0E11` | Deep base canvas with OLED depth |
| `panel_bg` | `#181B21` | `#121316` | Root container and panel surface |
| `widget_bg` | `#1E2229` | `#1A1B20` | Inactive buttons, search inputs, pills |
| `widget_hover` | N/A (gamma 1.1) | `rgba(255, 255, 255, 0.08)` | Subtle translucent illumination on hover |
| `widget_active` | `#262E44` (blue) | `rgba(255, 255, 255, 0.16)` | Crisp monochrome active state |
| `text` | `#D6DBE4` | `#EDEDEF` | Crisp off-white for primary text and body |
| `faint` | `#7A8191` | `#82838E` | Neutral graphite for secondary info and line numbers |
| `accent` | `#7C9EF9` (bright blue) | `#FFFFFF` / `#E5E5EA` | High-contrast pure white/silver primary accent |
| `accent_soft` | `#262E44` | `rgba(255, 255, 255, 0.12)` | Subtle highlight for selections |
| `link` | `#5CA6FF` | `#A1BCE0` | Muted slate ice blue with underline on hover |
| `code_bg` | `#1C1C22` | `#17181C` | Clean rounded container for inline code |
| `code_text` | `#FFC774` (yellow) | `#EDEDEF` | High-legibility monochrome code run |
| `code_fence_bg`| `#16181D` | `#141519` | Elevated block code container |
| `quote_bar` | `#5676C4` (blue) | `#6B6D76` | Sleek graphite vertical rule |
| `quote_bg` | `#1A1E28` | `#16171B` | Soft rounded quote callout |
| `hr` | `#3A404C` | `#22242B` | Hairline 1px border separator |
| `check` | `#58C484` | `#E5E5EA` / `#FFFFFF` | Minimalist checkbox glyph |
| `danger` | `#E06C77` | `#E06C75` | Restrained muted crimson |
| `warning` | `#E5C07B` | `#D4A259` | Restrained warm amber |
| `success` | `#98C379` | `#6EBE7F` | Restrained sage green |
| `match_bg` | `#B48C1E` (yellow) | `rgba(255, 255, 255, 0.22)` | Subtle inverted luminescence for find matches |

#### Markdown Syntax Tokens (Editor & Preview)
Replacing the multi-colored scheme with an editorial hierarchy:
- `md_heading`: `#FFFFFF` (pure white, high visual weight).
- `md_marker`: `#5A5B64` (receded graphite markers for `#`, `>`, `-`).
- `md_strong`: `#FFFFFF` (rendered with bold weight).
- `md_em`: `#C2C3C9` (italicized neutral).
- `md_link`: `#A1BCE0` (slate blue).
- `md_code`: `#EDEDEF` with `#17181C` background.
- `md_quote`: `#82838E` (neutral editorial quote).
- `md_list`: `#82838E` (neutral clean bullets/numbers).
- `md_task`: `#82838E` (bracket markers).

---

### 2.2 Light Palette ("Tesla Day / Notion Clean Slate")
Crisp, spacious, daylight reading canvas.

| Token | Current Value | Tesla/Notion Target | Visual Intent |
| :--- | :--- | :--- | :--- |
| `window_bg` | `#F8F9FB` | `#FBFBFD` | Clean, airy canvas |
| `panel_bg` | `#FFFFFF` | `#FFFFFF` | Pure white panel chrome |
| `widget_bg` | `#EEF0F4` | `#F1F2F4` | Soft pill backgrounds |
| `text` | `#242933` | `#111113` | Deep, crisp ink |
| `faint` | `#6E7684` | `#787B86` | Mid-gray for secondary labels |
| `accent` | `#485FC7` (blue) | `#111113` | Bold, solid black accent |
| `accent_soft` | `#E0E5FA` | `rgba(0, 0, 0, 0.08)` | Translucent dark gray selection |
| `link` | `#185CCD` | `#2E5AAC` | Refined navy slate |
| `code_bg` | `#F4F4F6` | `#F0F1F3` | Subtle light pill |
| `code_fence_bg`| `#F6F7FA` | `#F4F5F7` | Elevated light code block |
| `quote_bar` | `#728CD6` | `#9B9EA7` | Refined neutral bar |
| `quote_bg` | `#F0F3FB` | `#F7F8FA` | Crisp muted blockquote |
| `hr` | `#D6DBE4` | `#E5E7EB` | Subtle hairline line |

---

## 3. Custom Typography, Antialiasing & Rendering

### 3.1 Proportional Font Stack (UI Chrome & Markdown Content)
Rather than relying solely on default fallback glyphs, the font loader will probe system-installed modern sans-serifs in order of priority:
1. **Windows**: `Segoe UI Variable Text` / `Segoe UI Variable Display` -> `Segoe UI` -> `Arial`.
2. **macOS**: `SF Pro Text` / `SF Pro Display` -> `Helvetica Neue`.
3. **Linux**: `Inter` -> `Roboto` -> `Liberation Sans` -> `DejaVu Sans`.
4. **Fallback / Multilingual**: Keep existing CJK stack (`msyh.ttc`, `simhei.ttf`, `PingFang.ttc`, `NotoSansCJK-Regular.ttc`) intact.

### 3.2 Monospace Font Stack (Editor, Code Blocks, Metrics)
1. **Windows**: `Cascadia Code` -> `Consolas`.
2. **macOS**: `SF Mono` -> `Menlo`.
3. **Linux**: `JetBrains Mono` -> `Fira Code` -> `DejaVu Sans Mono`.

### 3.3 Antialiasing & Crisp Geometry Options
- Configure egui tessellation:
  ```rust
  ctx.tessellation_options_mut(|opt| {
      opt.feathering = true;
      opt.feathering_size_in_pixels = 1.0;
  });
  ```
- Configure typography line heights and paragraph spacing for a comfortable, Notion-like reading experience (body text at 14.0px with 1.45 relative line height).
- Strictly maintain `theme::register_bold_family` contract: any custom proportional font resolution must ensure `BOLD_FAMILY` (`"rustdown-bold"`) continues to resolve to genuine bold faces (`segoeuib.ttf`, etc.), preserving test suite invariants.

---

## 4. Component-by-Component Modernization Specs

### 4.1 Top Window Chrome & Menu Bar
- **Current**: Standard 26px native-style menu row.
- **Tesla/Notion Redesign**:
  - Borderless top bar with 1px hairline bottom divider (`pal.hr`).
  - Menu buttons ("File", "Edit", "View", "Help") rendered with 6.0px rounded hover highlights (`pal.widget_bg`) and subtle horizontal padding (8px).
  - Clean drop-down menus with 8.0px corner radius, 1px subtle stroke, and soft shadow.

### 4.2 Segmented Mode Switcher & Toolbar
- **Current**: Raw text buttons lined up together.
- **Tesla/Notion Redesign**:
  - **Segmented Capsule Control**: `[ View | Edit | Split ]` enclosed within a single rounded pill capsule (`corner_radius: 8.0px`, background `pal.widget_bg`). The active mode sits on an elevated pill (`corner_radius: 6.0px`, solid `pal.accent_soft` or `pal.panel_bg`) with crisp high-contrast text.
  - **Tool Groups**: Group formatting actions (Bold, Italic, Strikethrough, Code) and structure actions (Headings, Quotes, Lists, Tables) into distinct pill clusters with 6px corner radii, ghost styling by default, and subtle hover illumination.

### 4.3 Tab Bar
- **Current**: Plain text labels with basic "×" buttons separated by vertical dividers.
- **Tesla/Notion Redesign**:
  - Active tab rendered as an elevated pill (`corner_radius: 6.0px`, fill `pal.widget_bg`, text `pal.text`).
  - Inactive tabs rendered seamlessly with muted text (`pal.faint`) and hover brightening.
  - Unsaved indicator ("●") rendered as a crisp 5px geometric circle.
  - Close tab button ("×") rendered as a rounded micro-target that turns solid on hover.

### 4.4 Editor Canvas
- **Line Numbers**: Right-aligned muted monospace column with 8px right margin, cleanly delineated from code without distracting borders.
- **Padding & Breathing Room**: TextEdit given generous horizontal margin (14px) and line height to prevent cramped lines.
- **Caret & Selection**: Clean, high-contrast caret with subtle translucent selection highlight (`Color32::from_white_alpha(35)`).
- **Find Matches**: Elevated subtle luminescence overlay rather than garish yellow.

### 4.5 Preview Canvas (Notion-Style Reading Experience)
- **Content Max-Width**: Centered reading column with comfortable line length (max ~760px), matching Notion's document flow.
- **Headings**: Clean typographic hierarchy with bold weight, proportional tracking, and a subtle hairline rule for H1/H2.
- **Code Fences**: Elevated dark container (`#141519`), 8.0px rounded corners, subtle language badge in monospace, crisp syntax highlighting.
- **Blockquotes**: Left-accent bar (3.0px wide, `#6B6D76`), rounded background (`#16171B`), 8.0px radius, 12px inner margin.
- **Task Lists**: Sleek square checkboxes with 3.5px corner radius, hairline stroke when unchecked, smooth fill with clean white checkmark when checked.
- **Tables**: Modern borderless table with subtle horizontal separator lines and alternating subtle row striping.

### 4.6 Find & Replace Bar
- **Current**: Rigid row across the top panel.
- **Tesla/Notion Redesign**:
  - Rounded input fields (`corner_radius: 6.0px`, 1px hairline border).
  - Navigation buttons (↑ / ↓) as compact rounded icon buttons.
  - Counter badge (e.g. `2 of 8`) in subtle monospace faint text.

### 4.7 Status Bar
- **Current**: 22px status line with multi-colored indicators.
- **Tesla/Notion Redesign**:
  - Sleek 24px minimalist footer with 1px top hairline rule.
  - Monochrome status pill: "SAVED" in subtle faint text; "MODIFIED" with a subtle warm dot indicator.
  - Monospace telemetry (Ln/Col, encoding, zoom percentage) in uniform muted typography.

### 4.8 Dialogs & Modals
- **Current**: Standard egui Window with 100-alpha dark rect overlay.
- **Tesla/Notion Redesign**:
  - Backdrop: Uniform 50% dark overlay.
  - Modal Card: Floating card with 12.0px corner radius, hairline 1px border (`Color32::from_white_alpha(25)`), and deep obsidian fill.
  - Primary Action Buttons (e.g., "Save", "Go"): Solid white pill with black bold text (in Dark mode) or solid black pill with white text (in Light mode) — classic Tesla vehicle UI button styling.
  - Secondary Action Buttons (e.g., "Cancel", "Discard"): Subtle translucent pill with hover highlight.

### 4.9 Welcome / Empty State
- **Current**: Simple centered column with basic buttons.
- **Tesla/Notion Redesign**:
  - Tesla minimalist hero presentation:
    - Bold, minimalist title typography with generous vertical spacing.
    - Dual primary/secondary pill action buttons (`140px x 40px`, 8.0px corner radius).
    - Recent documents section with clean card-like hover states.

---

## 5. Architectural Implementation Phasing

When approval is granted, implementation will proceed through the following phased sequence:

```
[Phase 1: Theme & Font Foundation]
  ├── src/theme.rs: Update Palette tokens for Dark & Light modes
  ├── src/theme.rs: Update egui::Visuals and style defaults (corner_radius, item_spacing)
  └── src/app.rs: Extend install_fonts with modern system sans/mono font discovery

[Phase 2: Shell, Navigation & Chrome]
  ├── src/app.rs: Redesign top menu bar with rounded hover pills
  ├── src/app.rs: Implement segmented capsule control for Mode (View / Edit / Split)
  ├── src/app.rs: Update toolbar button clusters and icon sizing
  ├── src/app.rs: Redesign tab bar into Notion-style rounded pills
  └── src/app.rs: Redesign status bar into minimalist footer

[Phase 3: Editor & Preview Panes]
  ├── src/editor.rs & src/highlight.rs: Refine syntax highlighting into monochrome editorial theme
  ├── src/app.rs (edit_pane): Refine line numbers, margins, and selection metrics
  └── src/preview.rs: Refine blockquotes, code fences, checkboxes, headings, and table rendering

[Phase 4: Modals, Dialogs, Welcome & Toast]
  ├── src/app.rs (modal_ui): Apply Tesla card styling, 12px radii, and solid contrast buttons
  ├── src/app.rs (welcome): Modernize hero typography and action pills
  └── src/app.rs (toast_ui): Sleek rounded floating notification

[Phase 5: Automated & Visual Verification]
  ├── cargo test: Verify all 65 automated tests pass with 0 regressions
  ├── Headless and compiler checks: Ensure zero warnings and zero runtime panics
  └── Visual smoke check: Verify Dark/Light switching, rendering, and responsiveness
```

---

## 6. Safety Invariants & Risk Mitigation

1. **Zero Logic Changes**: All 65 existing tests in `src/` (buffer lifecycle, format toggles, smart enter, table operations, find/replace) must remain 100% green.
2. **Font Fallback Invariant**: Probing for modern fonts (`Segoe UI Variable`, `SF Pro`, `Inter`) must safely fall back if a font file is absent on a particular OS, guaranteeing that headless environments (e.g. CI) continue to run without panic.
3. **Strict Isolation**: All changes are confined to branch `feature/tesla-minimal-ui`.
4. **Reversibility**: Changes can be toggled or reverted cleanly if any visual nuance requires adjustment.
