# RustDownViewer — Features & Behavior Specification

**Version:** 0.2 (draft for Phase 0)
**Changelog:** v0.2 — added §3.6 (rich-content features F94–F109), §6.19–§6.23 (formatting toggles, tables, images/media, links, clipboard), elements E23–E26, §7.1 stress rows, decisions D11–D15.
**Status:** Baseline for implementation. Open decisions are flagged `[OPEN]`.
**Scope:** Feature catalog, interaction/behavior scenarios (clicks, double-clicks, repeated clicks, drags, hover, keyboard, and state-dependent edge cases), and acceptance criteria for a **native, cross-platform (Windows / macOS / Linux) Markdown viewer and editor written in Rust**.

---

## 1. Product summary & design principles

RustDownViewer is a desktop application to **read and edit Markdown files locally**.
It ships as a native app (no browser dependency, no server), respects privacy (no telemetry, files stay on disk), and treats **no data loss** as its primary invariant.

Design principles that drive every behavior decision below:

1. **One buffer, one truth.** Each open file maps to exactly one buffer object that owns the text, the dirty flag, the undo stack, and the file path. All views (edit pane, preview pane, tabs, status bar) read from that single buffer, so UI can never disagree about content.
2. **Every destructive action is reversible or guarded.** Close/replace/delete/reload paths always resolve to: *Save → Discard → Cancel*, and "discard" is never the default focus.
3. **Mouse and keyboard are peers.** Every click behavior has a keyboard equivalent, and every keyboard command works with the mouse. No behavior is reachable only one way.
4. **Predictable clicks.** Single click = primary action on the target. Double-click = a *secondary* action only where the target has two natural levels (word vs. line in text, select vs. open in a list). Triple-click stops at the third level. Anything beyond is accidental input and is ignored.
5. **Idempotent rapid repetition.** Clicking Save five times fast saves once effectively. Opening the same file twice focuses the existing tab instead of duplicating. Double-click must never be processed as "two single clicks with side effects."
6. **Graceful degradation.** If the app cannot do the ideal thing (render an exotic syntax, open a 2 GB file, watch a network drive), it degrades to a *documented* behavior rather than hanging or corrupting.
7. **State survives the user.** Unsaved content, window layout, and theme persist across restarts via an explicit crash-recovery flow.

---

## 2. Primary usage scenarios (who uses it and how)

| Scenario | User | Typical actions | Must not break |
|---|---|---|---|
| S1 Note taking | Casual writer | New file, type, autosave, search | Losing a keystroke on close |
| S2 Code/documentation review | Developer | Open README/docs, read rendered view, jump TOC, click links | Mis-rendering GFM tables/code |
| S3 Long-form writing | Blogger/author | Split view, word count, focus mode, export | Scroll desync between panes |
| S4 Doc maintainer | Tech writer | Multi-file workspace, find across files, external edits by git | Silent overwrite of files changed by git pull |
| S5 Markdown "clean-up" | Anyone | Edit raw source, syntax highlighting, find/replace, undo | Undo losing state after save |
| S6 Reading on the go / quick peek | Anyone | Open from file manager / CLI arg, view, close | App quitting when last tab closes |

---

## 3. Feature catalog

Priorities: **P0** = required for MVP v0.1.0 · **P1** = next milestone · **P2** = later / optional.

### 3.1 File management

| ID | Feature | Priority | Notes |
|---|---|---|---|
| F1 | Open file via native dialog | P0 | Multi-select supported |
| F2 | Open file via command-line argument | P0 | `rustdown file.md`, multiple paths, `--new`, `--line N` `[OPEN]` |
| F3 | Open by drag & drop onto window | P0 | Files → tabs; folder → workspace tree (see §6.13) |
| F4 | New file (untitled buffer) | P0 | Autosaved to recovery store until first Save As |
| F5 | Save (Ctrl/Cmd+S) | P0 | Writes to current path |
| F6 | Save As (Ctrl/Cmd+Shift+S) | P0 | Path dialog, keeps new path |
| F7 | Save a copy | P2 | Does not switch buffer path |
| F8 | Autosave | P1 | Default **off**? `[OPEN]`; see §8.5 |
| F9 | Recent files list (10) | P1 | Per-install, MRU order, cleared entries |
| F10 | Tabs (multi-file) | P0 | Reorder, close, middle-click close (§6.3) |
| F11 | Workspace / folder view (file tree) | P1 | Click-to-preview, double-click-to-open (§6.8) |
| F12 | Reveal file in OS file manager | P1 | Context menu |
| F13 | Watch external file changes | P0 | Auto-reload policy (§8.4) |
| F14 | File association ("Open with RustDownViewer") | P1 | Per-OS install step |
| F15 | Multi-window | P2 | One buffer can only be open in **one** window at a time `[OPEN]` |

### 3.2 Viewing / rendering

| ID | Feature | Priority | Notes |
|---|---|---|---|
| F20 | GitHub Flavored Markdown rendering | P0 | Headings, emphasis, lists, links, images, blockquotes, code, fenced code, tables, task lists, strikethrough, autolinks, hard/soft breaks per GFM |
| F21 | Syntax highlighting in code blocks | P0 | via `syntect`/similar; 40+ common languages |
| F22 | Inline code & code font styling | P0 | |
| F23 | Footnotes | P1 | Hover → popup; click → scroll to note and back |
| F24 | Math (KaTeX-style) | P1 | `$...$`, `$$...$$` |
| F25 | Mermaid / diagrams | P2 | Rendering engine decision `[OPEN]` |
| F26 | Table of contents sidebar (clickable anchors) | P1 | Generated from headings |
| F27 | Emoji / shortcode rendering | P2 | |
| F28 | Local & remote images | P0 | Local = file relative to md; remote = fetch w/ cache (offline fallback shown) |
| F29 | Themes: Light / Dark / System | P0 | System follows OS; dark first-class |
| F30 | Custom accent + font settings | P1 | Persisted per install |
| F31 | Zoom (Ctrl/Cmd +/-, 0) | P0 | Scales editor & preview text, not window |
| F32 | Word wrap toggle (preview) | P1 | Editor wraps independently (§3.3) |
| F33 | Export to HTML / PDF | P1 | HTML from same renderer; PDF via system print or embedded engine `[OPEN]` |
| F34 | Print | P1 | OS print dialog, paginated rendered output |
| F35 | Focus / readability mode (centered column) | P1 | |
| F36 | Reading progress indicator | P2 | |

### 3.3 Editing

| ID | Feature | Priority | Notes |
|---|---|---|---|
| F40 | Edit = raw Markdown source | P0 | Source editor, not WYSIWYG |
| F41 | Preview refreshes on edit | P0 | Debounced ~150 ms (§8.3) |
| F42 | Markdown-aware syntax highlighting in editor | P0 | Markup dimmed/colored vs. prose |
| F43 | Line numbers | P0 | Toggle |
| F44 | Current-line highlight | P0 | |
| F45 | Soft word wrap in editor | P0 | Toggle; visual-only, never rewrites file |
| F46 | Auto-pairing of `* _ \` [ ] ( ) { }` and quotes | P1 | Smart skip when typing the closing char |
| F47 | Tab / Shift+Tab indent & outdent; Tab inserts spaces (configurable) | P0 | Default: insert 2 spaces, no hard tabs written `[OPEN]` |
| F48 | Undo / Redo | P0 | **Unbounded** per buffer, survives Save (§8.6) |
| F49 | Find & Replace (in current file) | P0 | Replace-all undoable as one step |
| F50 | Find across workspace files | P2 | |
| F51 | Go to line | P1 | Ctrl/Cmd+G |
| F52 | Multi-cursor & column (Alt+drag) selection | P1 | Editor-engine gate `[OPEN]` |
| F53 | Word/line/paragraph selection by 2/3/4 clicks | P0 | §6.6 |
| F54 | Drag-selection with auto-scroll | P0 | |
| F55 | Markdown helpers toolbar/palette (insert heading, link, table, list) | P1 | Inserts at cursor; undoable |
| F56 | Spell check | P2 | |
| F57 | Read-only mode (View mode is not read-only editing of same buffer; modes defined in §6.5) | P0 | |

### 3.4 UI / shell

| ID | Feature | Priority | Notes |
|---|---|---|---|
| F60 | Three modes: **View / Edit / Split** (Ctrl/Cmd+1/2/3) | P0 | §6.5 |
| F61 | Tab bar with dirty indicator, close buttons | P0 | §6.3 |
| F62 | Status bar: dirty state, cursor Ln/Col, word count, encoding, file path (hover), zoom % | P0 | Clickable items §6.11 |
| F63 | Command palette (Ctrl/Cmd+K / Ctrl/Cmd+P) | P1 | Fuzzy action search |
| F64 | Context (right-click) menus everywhere meaningful | P0 | |
| F65 | Keyboard shortcut cheat sheet (built-in, `?`) | P1 | |
| F66 | Native title bar & window controls | P0 | OS-drawn; double-click title bar = OS maximize |
| F67 | Empty state ("welcome") screen | P0 | §6.14 |
| F68 | Startup recovery banner after crash | P0 | §8.7 |
| F69 | About / version / licenses dialog | P1 | |
| F70 | Localization (i18n) | P2 | English first |

### 3.5 Reliability, performance, cross-platform

| ID | Feature | Priority | Notes |
|---|---|---|---|
| F80 | Atomic save (temp file + rename on same volume) | P0 | Preserves file on crash/power loss |
| F81 | CRLF / LF preservation & conversion | P0 | File keeps its own line endings; new files default to OS convention; setting to convert on save |
| F82 | BOM handling (UTF-8 BOM preserved or stripped per setting) | P0 | |
| F83 | Non-UTF-8 detection (UTF-16, Latin-1…) | P1 | Open with encoding picker; never corrupt on save |
| F84 | Large file strategy (lazy load / virtualize / warn) | P0 | §9 |
| F85 | Read-only file / permission handling | P0 | §8.8 |
| F86 | File deleted/moved while open | P0 | §8.4 |
| F87 | Multiple instances locking | P2 | Warn if same file opened by second instance |
| F88 | HiDPI & fractional scaling | P0 | |
| F89 | System theme & accessibility settings honored | P1 | |
| F90 | Accessibility: full keyboard nav, focus rings, ARIA/AT-SPI labels where native, high-contrast theme | P1 | |
| F91 | Crash-safe recovery store | P0 | §8.7 |
| F92 | App config store | P0 | Per-install JSON/TOML in OS config dir; atomic writes |
| F93 | Unit + UI test harness | P1 | Headless renderer snapshots, event-script tests |

---

### 3.6 Rich-content authoring UX (formatting, tables, images, clipboard)

Detailed interaction contracts live in §6.19–§6.23; this catalog lists the features (F94+ refine and detail F55).

| ID | Feature | Priority | Notes |
|---|---|---|---|
| F94 | Formatting commands: Bold / Italic / Strikethrough / Inline code (toolbar + shortcuts) | P0 | Unified inline-toggle contract §6.19; doubled markers such as `****text****` are unreachable |
| F95 | Block formatting: headings H1–H6, blockquote, bullet/numbered/task lists, fenced code | P0 | Line-aware, repeat-to-remove toggles; §6.19 |
| F96 | Insert / edit / remove link (`Ctrl/Cmd+K`) | P0 | Placeholder flow; inline popover P1; §6.22 |
| F97 | Insert table via rows×columns grid picker | P0 | §6.20 |
| F98 | Table structure ops: add/remove row & column, align, move row/column | P1 | Undoable whole-block realign; §6.20 |
| F99 | `Tab` / `Shift+Tab` table-cell navigation + row append on last cell | P1 | §6.20 |
| F100 | Insert image via dialog with relative-path resolution | P0 | §6.21 |
| F101 | Paste image from clipboard → save asset + insert reference | P1 | Media-folder policy D11; §6.21 |
| F102 | Drag & drop an image onto the editor → insert at drop point | P1 | §6.21 |
| F103 | Copy image (file bytes) from editor/preview; Copy Path; Reveal | P1 | Extends §6.7 preview menu; §6.21 |
| F104 | Copy as Markdown from preview (rendered → source Markdown, never HTML) | P1 | §6.23 |
| F105 | Smart paste (HTML / rich / TSV → Markdown incl. GFM tables) | P1 | Default OFF (`[OPEN]` D13); §6.23 |
| F106 | Media-folder management & unused-asset cleanup | P2 | Undo never deletes files; §6.21 |
| F107 | Inline link/image popover editor (Text / URL / Title / Alt fields) | P1 | §6.22 |
| F108 | Empty-pair placeholder auto-clear | P1 | §6.19 |
| F109 | Toolbar formatting active-state indicators | P1 | §6.19 |

---

## 4. Global interaction vocabulary (used by every scenario below)

A click means **mouse button down + up without significant movement** on the same target. The app distinguishes:

| Event token | Meaning |
|---|---|
| `click` | Single primary (left) click |
| `dblclick` | Two primary clicks, second within **500 ms** and within a **4 px** slop radius |
| `tripleclick` | Three clicks, same window/distance rules as double |
| `quadclick` | Four clicks (rare, text only) |
| `rclick` | Secondary (right) click → context menu |
| `mclick` | Middle click |
| `hover` | Pointer over target without buttons |
| `drag` | Press + move beyond slop (≈4 px) then release |
| `press` / `release` | Down / up in isolation (used for drag, scrollbar thumb) |
| `wheel` | Vertical scroll; `Shift+wheel` horizontal; `Ctrl+wheel` zoom |
| `clickX` | Click on a target labeled **X** (a close button), used generically in this spec |

**Global click-count rules (invariants):**
- G1. The click counter resets on: mouse move beyond 4 px, release outside the target, key press, focus change, or 500 ms of inactivity.
- G2. A double/triple click is **one gesture**, not N single clicks: side effects of single clicks are suppressed until the gesture resolves. (Example: in the file tree, a double-click must not first trigger the single-click preview action with its own side effect; the tree defers single-click action by ~150 ms — see §6.8.)
- G3. Any single gesture is **idempotent by target**: clicking Close twice rapidly while the first close is awaiting a dialog does not stack two dialogs (§8.1).
- G4. Hover never mutates state (tooltips/status text only).
- G5. A right-click does not move the caret in the editor (Windows convention) but does select the word under the pointer for its context menu target `[OPEN]`.

**Modifier conventions (macOS shown after `/`):**

| Shortcut | Windows / Linux | macOS |
|---|---|---|
| Save | Ctrl+S | Cmd+S |
| Close tab | Ctrl+W | Cmd+W |
| Close window/app | Ctrl+Shift+W | Cmd+Shift+W |
| New file | Ctrl+N | Cmd+N |
| Open | Ctrl+O | Cmd+O |
| Zoom in/out/reset | Ctrl+= / Ctrl+- / Ctrl+0 | Cmd+= / Cmd+- / Cmd+0 |
| Find / Replace | Ctrl+F / Ctrl+H | Cmd+F / Cmd+Alt+F |
| Go to line | Ctrl+G | Cmd+L |
| Command palette | Ctrl+Shift+P | Cmd+Shift+P |
| Mode View/Edit/Split | Ctrl+1 / Ctrl+2 / Ctrl+3 | Cmd+1 / Cmd+2 / Cmd+3 |

---

## 5. UI element inventory

Every interactive element in the app and where its behaviors are specified:

| # | Element | Location | Spec section |
|---|---|---|---|
| E1 | Window close / minimize / maximize controls | Title bar | §6.1 |
| E2 | Window title bar (drag region, double-click) | Top | §6.2 |
| E3 | Tab (file) | Tab bar | §6.3 |
| E4 | Tab close button (X) | Each tab | §6.3 |
| E5 | Tab bar empty area / "+" new-tab | Tab bar | §6.3 |
| E6 | Mode switcher (View/Edit/Split) | Toolbar / status | §6.5 |
| E7 | Editor text surface | Edit / Split mode | §6.6 |
| E8 | Preview surface (rendered HTML) | View / Split mode | §6.7 |
| E9 | Scrollbars (both panes) | Panes | §6.10 |
| E10 | Split divider | Split mode | §6.9 |
| E11 | File tree rows | Sidebar | §6.8 |
| E12 | Sidebar toggle & resize edge | Sidebar | §6.8 |
| E13 | Status bar items | Bottom | §6.11 |
| E14 | Menus (app/file/edit/view…) | Menu bar | §6.12 |
| E15 | Toolbar buttons | Optional top bar | §6.12 |
| E16 | Dialog buttons (Save/Discard/Cancel, OK…) | Dialogs | §6.12 / §8.1 |
| E17 | Find & Replace bar | Below toolbar | §6.16 |
| E18 | Links & images inside preview | Preview | §6.7 |
| E19 | TOC / heading list | Sidebar (preview) | §6.15 |
| E20 | Empty-state / welcome cards | Main area | §6.14 |
| E21 | Command palette input & results | Overlay | §6.17 |
| E22 | Notifications / toasts | Corner overlay | §6.18 |
| E23 | Formatting toolbar (B / I / S / inline code / block menu) | Toolbar (Edit & Split modes only) | §6.19 |
| E24 | Table tools: insert grid picker + contextual row/column controls | Editor overlay / context menu | §6.20 |
| E25 | Link & image insertion flows (placeholders / popovers) | Editor overlay | §6.21 / §6.22 |
| E26 | Clipboard paste surface (text / rich / image) | Focused editor pane | §6.21 / §6.23 |

---

## 6. Behavior & scenario specifications

> For each element: **Event / Condition → Behavior**. "Normal state" means no dialog open, buffer not busy. Behaviors marked `[verified visually]` are acceptance-check items for the UI test suite in Phase 5.

### 6.1 Window controls (E1) — the "X" of the window

Applies identically to: the OS close button (X), `Alt+F4` (Win/Linux), `Cmd+Q` (macOS) exit path, and menu **File → Exit**.

| Event / Condition | Behavior |
|---|---|
| `clickX` window, **no dirty buffer anywhere** | App quits immediately. All buffers were clean → nothing to lose. |
| `clickX` window, **≥1 dirty buffer**, autosave **off** | Modal dialog: *"Save changes to «file.md»? — Save · Discard · Cancel"*. App does **not** close. Default focus = **Save**. |
| … user picks **Save** | All dirty buffers saved (each Save As dialog if untitled), then app quits. |
| … user picks **Discard** | Buffers discarded, app quits. **Never the default; requires deliberate second click** (Discard is not highlighted; user must Tab/click to it). |
| … user picks **Cancel** / presses `Esc` / `clickX` again | Dialog closes, app stays open, nothing changed. |
| `clickX` window **while a dirty-buffer dialog is already open** (rapid second click, G3) | Ignored. Exactly one modal instance exists app-wide (§8.1). |
| `clickX` window, buffer **dirty but autosave on** and recovery store healthy | No dialog. Autosave flushes synchronously at quit; app quits. (§8.5) |
| `clickX` window with **only the last tab open, buffer clean** | **App quits.** Contrast with tab X (§6.3): window close ≠ tab close. |
| Save in progress (large file) when X clicked | X is ignored until write completes; status bar shows "Saving…"; then the same close logic runs on the now-clean buffer. |
| Window minimized then closed from taskbar/dock | Same as X path above (OS "close" event) |
| OS shutdown/logout while app open | App receives close event → same dirty-guard dialog; if shutdown can't wait, autosave/recovery store is flushed first and dialog is skipped with a recovery banner on next start (§8.7). |

### 6.2 Window title bar (E2)

| Event | Behavior |
|---|---|
| `drag` on title bar | Moves the window (native behavior). Never conflicts with double-click because drag needs >4 px movement first (G1). |
| `dblclick` on title bar | OS standard: toggle maximize/restore (Windows/Linux), zoom (macOS). RustDownViewer does **not** override. |
| `dblclick` on title bar while maximized | Restore to previous size/position. |
| `hover` on title bar | Shows full path of the active file as tooltip when the title truncates it. |
| `rclick` on title bar | OS window menu (Restore/Move/Size/Minimize/Maximize/Close) — native, unmodified. |
| Title bar text | `«filename.md» — RustDownViewer`; `●` (or `•`) prefix + italic when dirty. Live-updates on buffer rename/save. |

### 6.3 Tabs (E3) & tab close button (E4) & tab-bar chrome (E5)

**Tab X click and double-click are the canonical "click X or Y twice" cases — specified here explicitly.**

| Event / Condition | Behavior |
|---|---|
| `click` on a tab | Activates it (switches buffer view). If it was already active → no-op (focus stays). |
| `click` on tab **X** (close), buffer **clean** | Tab closes immediately. |
| `click` on tab **X**, buffer **dirty**, autosave off | Modal: *"Save changes to «file.md» before closing? Save / Discard / Cancel"*. Default = Save. |
| … **Save** | If untitled → Save As dialog first; then tab closes. If Save As is cancelled → tab **stays open**, dialog gone. |
| … **Discard** | Tab closes, buffer content lost (user chose it). |
| … **Cancel** | Tab stays open. |
| `clickX` **twice rapidly** on the same tab (double click on the X) | First click opens the dirty-guard dialog; second click is **ignored** (G3). The gesture must not close the tab behind the dialog. |
| `dblclick` on a tab (body, not X) | **No-op** (reserved). Rationale: in a tab strip, double-click has no standard second-level meaning and would conflict with accidental rapid switching. Single click already selects. *(VS Code/browsers agree: dblclick on tab body is inert.)* |
| `dblclick` on **empty area of tab bar** | Opens a **New file** (untitled) tab — browser muscle memory. |
| `click` on "+" button at tab-bar end | Same: new untitled tab. |
| `mclick` on a tab | Closes it **without** dialog **only if clean**; if dirty → same Save/Discard/Cancel dialog as X. |
| `drag` a tab sideways | Reorders tabs (visual drop indicator). Buffer content untouched. Drop on same position = cancel. |
| `drag` a tab **out** of the bar | P2: detach to a new window. In P0/P1 the drag reverts (springs back) with no action. |
| `rclick` on a tab | Context menu: Close · Close Others · Close All · Copy Full Path · Reveal in File Manager · (dirty: Save). `Close All` with mixed clean/dirty → single multi-file dialog listing dirty files (§8.1). |
| `rclick` on tab X | Same as click on X (dialog path), menu suppressed. |
| `hover` on tab (no click) | Tooltip: full file path + modified time + encoding. Dirty tab shows `●` dot; **no** action on hover (G4). |
| `wheel` over tab bar | Cycles active tab left/right (horizontal scroll if overflow). |
| Keyboard `Ctrl+Tab` / `Ctrl+Shift+Tab` (or `Ctrl+PgUp/PgDn`) | Next / previous tab (MRU order). `Ctrl+W` = close active tab via same dirty guard. |
| **Last tab closed** | Tab closes; window **stays open** showing the Empty State (§6.14). App quits only via window X (§6.1). |
| Open same file that is already open in another tab | No duplicate tab: existing tab is activated (and, if a Save dialog for it is open, that dialog is raised). |
| Tab count exceeds visible width | Tabs compress to icon (dirty dot + file type glyph) + scroll arrows appear; active tab always visible. |

### 6.4 Window menu bar (E14 — app-level, macOS in the app menu)

| Event | Behavior |
|---|---|
| `click` menu name | Opens dropdown. `hover` over another menu name while open | switches menu (standard). `Esc` / click outside / click menu name again | closes, no action. |
| Menu items map 1:1 to commands (palette, shortcuts) — no menu item exists without a keyboard equivalent. |
| **File**: New · Open… · Open Recent ▸ · Save · Save As… · Save a Copy… · (P1 Export ▸ HTML/PDF) · Close Tab · Close Window · Exit |
| **Edit**: Undo · Redo · Cut/Copy/Paste (native clipboard) · Find ▸ · Replace ▸ · Select All · Go to Line |
| **View**: Mode ▸ View/Edit/Split · Sidebar · TOC · Word Wrap · Zoom ▸ · Theme ▸ Light/Dark/System · Full Screen (F11) |
| Disabled menu items render disabled with reason on hover tooltip (e.g., Paste disabled when clipboard empty is OS-handled; Copy disabled when nothing selected). |

### 6.5 Modes (E6): View / Edit / Split

Three modes operate on the **same buffer** — switching never loses content or caret state.

| Mode | Layout | Editing allowed? |
|---|---|---|
| **View** | Rendered preview full area; sidebar TOC available | No text editing (this is "reading"). |
| **Edit** | Raw source full area | Yes. |
| **Split** | Editor left, live preview right, draggable divider | Yes. |

| Event / Condition | Behavior |
|---|---|
| `click` View/Edit/Split control (or `Ctrl/Cmd+1/2/3`, or menu) | Switches mode instantly. Editor caret position, scroll positions of both panes, undo stack, and selection are preserved across the switch. |
| `click` same mode button that is already active | No-op (idempotent, G3). |
| Split mode, user edits in left pane | Right preview re-renders after the ~150 ms debounce (§8.3), preserving preview **scroll anchor** (§8.2). |
| Split mode, user scrolls editor | Preview does **not** auto-scroll (independent scroll, P1: optional "sync scroll" toggle that follows caret heading). |
| P1 sync-scroll ON: caret in editor over heading "## X" | Preview scrolls so "## X" is at top; further preview scroll disables sync until next caret move past threshold. |
| In View mode user presses any text key | Ignored with a transient toast *"View mode — press Ctrl/Cmd+2 to edit"* (never swallowed silently). |
| Buffer dirty and user switches modes | Mode switch is never blocked by dirty state (no data at risk — same buffer). |
| `Ctrl+1/2/3` pressed twice rapidly | Second press is a no-op (mode already active). No toggle-flip-flop. |

### 6.6 Editor text surface (E7)

**Selection & caret gestures:**

| Gesture | Behavior |
|---|---|
| `click` | Places caret at click point (snaps to nearest glyph). Moves the scroll view if click is near edge during drag-select. |
| `click` at a point while a selection exists (no modifier) | Collapses selection to caret at click point. |
| `dblclick` on a word | Selects the word (Markdown-aware: `**bold**` double-click selects `bold` inside markers `[OPEN]` refinement). |
| `tripleclick` on a line | Selects the whole logical line (excludes EOL). |
| `quadclick` on paragraph | Selects paragraph. *(4th level = stop: any further clicks in the gesture re-select paragraph, no side effect.)* |
| `dblclick` then `drag` | Word-by-word extending selection. |
| `tripleclick` then `drag` | Line-by-line extending selection. |
| `press` + `drag` | Character selection with **auto-scroll** at edges (viewport scrolls while dragging near top/bottom). Release ends selection. |
| `Alt+drag` (P1) | Column (rectangular) selection where engine supports it. |
| `Shift+click` | Extends selection from caret/selection anchor to click point. |
| `rclick` | Does **not** move caret by default (Windows). Context menu: Cut/Copy/Paste, Select All, (selection-aware) Copy as HTML/Markdown (P1), spell suggestions (P2). |
| `rclick` on a misspelled word (P2) | Same menu plus suggestions. |
| `wheel` | Scrolls vertically. `Shift+wheel` → horizontal. `Ctrl+wheel` → zoom (§ zoom). |
| `click` line-number gutter (F43) | Selects that line (same as triple-click). |
| `drag` gutter | Selects contiguous lines. |
| `dblclick` gutter | Selects the whole paragraph or file `[OPEN]`; safe default: same as line select (no extra level). |
| `hover` gutter | Line number highlighted; drag-resize edge appears at gutter/pane boundary (drag = widen gutter). |
| Click between text and right edge beyond wrapped line | Caret goes to end of that visual line (smart home/end conventions). |
| Typing while selection active | Typing replaces the selection; undo records the replacement as one step. |
| Typing while **composing IME** (CJK etc.) | IME composition events are passed through; preview does not re-render mid-composition, only after commit. |

**Input / editing keys:**
- `Enter` inserts newline + carries indent (markdown lists: auto-indent `- ` / `1. ` continuation P1; blank line ends the auto-list).
- `Backspace` removes selection or char; with auto-pairing on, deleting an opening delimiter also removes a matching adjacent closer (smart backspace, P1).
- `Tab` inserts configured indent (2 spaces default); with a multi-line selection, indents all lines; `Shift+Tab` outdents.
- `Ctrl/Cmd+Z` undo, `Ctrl/Cmd+Shift+Z`/`Ctrl+Y` redo. Undo never clears on save (§8.6).
- Double-press of a shortcut with a guard (e.g., Ctrl+S while saving) is coalesced (§8.5).

### 6.7 Preview surface (E8) — links, images, interactive tokens

| Element / Event | Behavior |
|---|---|
| `click` on a **link** | Opens in the **default OS browser**. Never opens inside the app. |
| `dblclick` on a link | Same as click (idempotent, G2: first click already fired). No different second-level meaning. |
| `ctrl/cmd+click` on a link | Same as click (uniform). |
| `rclick` on a link | Menu: Open Link · Copy Link Address · Copy Link Text. |
| `hover` on a link | Status bar shows the target URL; tooltip after 600 ms. G4 (no action). |
| `click` on **relative link to another .md** | P1: if file exists, opens it in a new tab; else toast "Linked file not found: path". |
| `click` on **anchor link** `#heading` (P1 TOC/anchors) | Scrolls preview to that heading; heading gets momentary highlight. |
| `click` on **image** | Selects it (outline). |
| `dblclick` on image | Opens the image **in the OS default viewer** (P0) — no in-app zoom engine. |
| `ctrl/cmd+click` on local image | Reveals containing folder in file manager. |
| `rclick` on image | Menu: Open in Viewer · Reveal in Folder · Copy Path · (P1) Copy Image. |
| `hover` broken image / missing file | Broken-image glyph; tooltip "Cannot load: path". |
| `click` on **footnote marker** (P1) | Scrolls to the footnote definition; a "back" arrow returns. `Esc` cancels the jump-back state. |
| `click` on **task-list checkbox** (rendered) | P1: toggles the underlying `[ ]`/`[x]` in the buffer (edit-through-preview), marks buffer dirty, undoable. In View mode (read-only) it does nothing. |
| `click` on **collapsible** (`<details>` passthrough) header | Expands/collapses natively in rendered view (CSS-only toggle). |
| `click` **inside a code block** | No action (code is selectable text — drag selects). Double-click selects word. |
| Copy button on code blocks (P1) | Click = copy block to clipboard with transient "Copied ✓" state. |
| `wheel` over preview | Scrolls preview. `Ctrl+wheel` zooms text. |
| Selection in preview then **Copy** | Copies plain text (markdown source of the selection if it was produced by the renderer — never copies HTML). |

### 6.8 File tree sidebar (E11, E12) — single vs double click semantics

Classic **"single-click = preview, double-click = commit"** model (VS Code/Atom-style), chosen so keyboard users and mouse users have one rule and no double-fire (G2).

| Event / Condition | Behavior |
|---|---|
| `click` on a **file row** | Row highlights; file loads into the **preview slot** (replaces whatever the preview pane shows) **after a 150 ms defer** — the defer exists purely so a fast second click is swallowed into a double-click (G2). Buffer is not "opened" yet (not pinned, no tab). |
| `dblclick` on a **file row** | Opens the file as a **pinned tab** (full open). If the single-click preview already loaded it, no reload happens — same buffer promotes to a tab. |
| `click` on a **folder row** (body) | Toggles expand/collapse of children. |
| `click` on folder **chevron ▸** | Expands/collapses (same as body click). |
| `dblclick` on folder | Same as click (expand/collapse) — folder has no second level. No double-fire. |
| `drag` file row onto editor/preview area | Opens that file in the target tab context (drop on editor = replaces active buffer). |
| `rclick` file row | Menu: Open · Open in New Tab · Reveal in Folder · Copy Path · Rename… · Delete… (both guarded: rename over existing file asks to overwrite; delete asks confirmation, is not undoable) |
| `rclick` folder row | Menu: New File… · New Folder… · Rename · Delete · Reveal · Collapse All |
| `click` a row that is **already selected** | No-op (stays selected). |
| `dblclick` the **already-selected row's file already pinned open** | Focuses its tab. |
| Keyboard: ↑/↓ move selection (live preview follows), `Enter` = open pinned tab, `F2` rename, `Del` delete (guarded). |
| `drag` sidebar divider | Resizes sidebar (min 160 px, max 45% of window). |
| `dblclick` sidebar divider | Resets to default width (280 px). |
| `click` sidebar toggle button (E12) | Hides sidebar (content area grows). When hidden, active file's tab remains. |
| Auto-refresh: external folder changes | Tree refreshes on window focus and on a 2 s background watcher; expanded nodes preserve their expansion state on refresh. |
| File tree shows git status coloring only if inside a repo and P1 — never blocks on git command (timeout 500 ms, then plain icons). |

### 6.9 Split divider (E10)

| Event | Behavior |
|---|---|
| `drag` divider | Resizes editor/preview ratio. Double-click distance guard applies (drag needs >4 px). |
| `dblclick` divider | Resets to **50/50**. Idempotent (second dblclick while already 50/50 = no-op). |
| Keyboard `Ctrl/Cmd+Shift+[` / `]` | Moves divider focus in 10 px steps (P1, accessibility). |
| Side pinned: when a file is open **without** split (View/Edit) | No divider exists. Entering Split restores last used ratio (persisted per window). |

### 6.10 Scrollbars (E9) & scrolling

| Event | Behavior |
|---|---|
| `drag` thumb | Scrolls proportionally; content follows 1:1. |
| `click` track above/below thumb | Page up/down (standard). |
| `dblclick` track | Same as single click on track — two rapid page-jumps are *intended* (both fire; no side effect exists) so no swallow needed. |
| `click` arrows (if drawn) | Small step. Press-and-hold repeats at 50 ms until release or end reached. |
| `wheel` | Line-scroll (editor) / smooth scroll (preview) — both honor OS "scroll lines" setting. |
| `Shift+wheel` | Horizontal scroll (wrapped-text panes no-op). |
| `Ctrl/Cmd+wheel` | Zoom text (never scrolls). Clamps 50%–400%. |
| Scroll while a **dialog** is open | Scrolls the dialog if scrollable; underlying pane scroll is locked (modal). |
| Scroll at document end | Haptic/bounce where the OS provides it; otherwise hard stop. No auto-load of more content (whole file is in memory per §9 strategy). |
| `click` and hold thumb then press `Esc` | Cancels the drag (thumb returns, no jump). |

### 6.11 Status bar (E13)

| Item | Click behavior |
|---|---|
| Branch icon: `● Modified` / `Saving…` / `Saved` (or dirty dot) | `click` → if dirty: triggers Save. If saving: disabled (idempotent). If clean: tooltip "All changes saved". |
| `Ln X, Col Y` | `click` → opens Go-to-Line prompt pre-filled. `Enter` jumps, `Esc` closes. |
| `Words: N · Chars: M` | `click` → toggles a small statistics popover (lines, paragraphs, reading time). Click again/outside closes. |
| Encoding (`UTF-8`, `UTF-8 BOM`, `UTF-16LE`…) | `click` → menu: **Reopen with…** (discard current decode, reload) and **Convert & save as…** (rewrites bytes). Both guarded by dialogs where data would change. |
| Line endings `LF`/`CRLF` | `click` → menu to view/convert; converting marks buffer dirty (undoable). |
| Zoom `%` | `click` → menu 50/75/100/125/150/200/300/400; `dblclick` → reset to 100%. |
| Path breadcrumb (P1, file tree context) | `click` segment → navigates tree; `rclick` → copy path. |
| Persistent error state | Shows inline (e.g., "Failed to save — permission denied") and is itself clickable → reopens the save attempt or the error detail. |

### 6.12 Dialog & toolbar buttons (E15, E16)

| Event / Condition | Behavior |
|---|---|
| `click` a toolbar **action** button | Runs command. Buttons are disabled while their command is invalid (Save disabled when clean). |
| `dblclick` any toolbar/menu **action** button | Two executions of the same command — for idempotent commands (Save, mode switch) the second is a no-op; for toggle commands (bold, wrap) the second execution **re-toggles to original state** — so the app treats double-click as two intentional single clicks here (no side-effect risk) except where a guard dialog exists (§8.1). |
| `click` dialog **Save** | Confirms; dialog closes. |
| `click` dialog **Discard** | Requires it to be a deliberate separate click (never keyboard default). Executes. |
| `click` dialog **Cancel** / `Esc` | Cancels. |
| `click` the **X** of a dialog window | Same as Cancel — **never** same as Discard/Delete. |
| `clickX` **twice** on dialog X | First = cancel (dialog closes); second click lands on whatever is beneath (e.g., app window) — normal, no queued behavior. |
| Dialog open + user clicks underlying UI | Modal blocks it (dialog is application-modal). Underlying click is discarded, not queued. |
| `Enter` in a dialog | Activates the default button (always the safe one: Save / OK / Cancel-where-default). |
| Any destructive dialog | The destructive button (Discard/Delete/Overwrite/Reload-losing-changes) is: rightmost, red-tinted, **not** default, and requires click (or Tab+Space). |

### 6.13 Drag & drop onto window (F3)

| Drop target / Condition | Behavior |
|---|---|
| Drop **.md file(s)** on empty main area | Opens each as a new tab (first becomes active). |
| Drop **.md file** on an existing tab | Opens in **that** tab (replaces its buffer). If the target buffer is dirty → Save/Discard/Cancel dialog **first**; Cancel aborts the whole drop. |
| Drop **.md file** on editor pane | Opens in the active tab (same dirty guard). |
| Drop **folder** on window | Opens the workspace sidebar rooted at that folder (no files auto-opened; tree shown with the folder selected). |
| Drop **non-markdown file** | P2: attempts plain-text open with a "binary?" sniff warning. P0: toast "Unsupported file type: «name»" and no state change. |
| Drop **same file as an already-open tab** | No duplicate: focuses existing tab (§6.3). |
| Drag enters window | Shows drop affordance overlay ("Open N file(s)"). Drag leaves | overlay clears. |
| Drop while a modal dialog is open | Ignored (modal lock). |
| Drop during an active save write | Queued until write completes, then processed normally. |

### 6.14 Empty state / welcome screen (E20)

| Event | Behavior |
|---|---|
| `click` **Open File…** card/button | Native open dialog (multi-select). |
| `click` **New File** card | Creates untitled buffer (goes to Edit mode). |
| `click` a **Recent** row | Opens that file. Row shows full path on hover; `rclick` row → Remove from Recent · Copy Path. |
| `dblclick` any card/row | Same as click (cards have one level; G2-swallow applies where the open dialog would otherwise fire twice — open is guarded to a single instance anyway). |
| `click` a **template** card (P1: README, notes, meeting notes) | Creates untitled buffer pre-filled with template text. |
| Empty state with zero recents | Recent section hidden entirely. |
| `click` outside any card | Deselects card highlight (nothing else). |

### 6.15 TOC sidebar (E19, P1)

| Event | Behavior |
|---|---|
| `click` heading row | Scrolls preview so heading is at top; heading highlighted; row becomes active. |
| `dblclick` heading row | Same as click (no second level; swallow applies). |
| `click` collapse caret next to a heading with children | Expands/collapses subtree (state persisted per session). |
| `hover` heading row | Reveals an "anchor link" icon → click copies `file.md#slug` to clipboard. |
| Clicking TOC **after editing changed headings** | TOC rebuilds on the render debounce; active row may vanish → selection falls back to nearest still-existing ancestor. |

### 6.16 Find & Replace bar (E17)

| Event / Condition | Behavior |
|---|---|
| `click` find input / `Ctrl/Cmd+F` | Opens bar (pre-filled with current selection if any), focus into query, all matches highlighted. |
| `Enter` | Next match. `Shift+Enter` | previous. Wraps around with a subtle "wrapped" indicator. |
| `click` **↑/↓** arrows | Next/previous match (same as Enter/Shift+Enter). |
| `Alt+Enter` / **Replace** button | Replaces current match, advances to next. |
| **Replace All** | One atomic operation: **single undo step**, and only after a confirm if count > 100 (`[OPEN]`: threshold). Reports "Replaced N occurrences" toast. |
| `click` match-highlight in editor | Caret jumps to that match. |
| `click` **X (close)** on bar | Closes bar; selection/highlights cleared; caret stays where it was. |
| `clickX` twice rapidly on bar X | First closes; second is a normal click on whatever is beneath. No ghost state. |
| `Esc` | Closes bar (first Esc if query editing and not yet closed: clears query selection, second Esc closes — single Esc closes `[OPEN]`; default: single Esc closes). |
| Regex mode toggle (P1) | Invalid regex → inline error, no crash, matches frozen at last valid state. |
| Typing while bar open | Filter updates live with debounce 100 ms; editor content untouched. |
| Buffer edited while matches visible | Matches recompute on the same debounce; stale highlights never persist. |
| Wrap toggle OFF and last match reached | Enter at last match = stays + status hint "End of file reached". |

### 6.17 Command palette (E21, P1)

| Event | Behavior |
|---|---|
| `Ctrl/Cmd+Shift+P` | Opens overlay, query focused. Re-press while open | closes it (toggle). |
| Typing | Fuzzy-filters commands + files (workspace). ↑/↓ move, `Enter` executes, `Esc` closes, `click` outside closes. |
| `click` a result | Executes it. `dblclick` a result | First click executed already → second click is on a closed overlay (click falls through to app). *Mitigation:* palette closes on `mousedown`, so double-click's second press happens on the app — safe (G2 design note). |
| Commands that need arguments (Save As, Go to Line) | Execute then immediately open their argument dialog. |

### 6.18 Notifications / toasts (E22)

| Event | Behavior |
|---|---|
| Toast appears | Non-modal, auto-dismisses after 4 s (errors persist until acknowledged or auto-dismiss at 10 s). |
| `click` toast body | Dismisses it (or opens the action it advertises, e.g., "Retry save"). |
| `click` toast **X** | Dismisses. Double X = first dismisses, second hits app (normal). |
| `hover` toast | Pauses auto-dismiss timer. |
| Actionable toast with a button | Button uses §6.12 rules. |

### 6.19 Formatting commands — inline styles & block tools (E23)

Formatting buttons live on a toolbar visible in **Edit** and **Split** modes only (hidden/disabled in View per §6.5). Generic toolbar button behavior (incl. double-click = two intentional executions) comes from §6.12; this section defines what **each execution does**. Keyboard equivalents: Bold `Ctrl/Cmd+B` · Italic `Ctrl/Cmd+I` · Strikethrough `Ctrl/Cmd+Shift+X` · Inline code `Ctrl/Cmd+Shift+backtick` · Insert link `Ctrl/Cmd+K` (keymap `[OPEN]` — see D15). Commands are disabled while a modal dialog is open (§8.1) and always enabled while a buffer is editable even if the file is read-only on disk (§8.8).

**Inline styles — Bold `**…**`, Italic `*…*`/`_…_`, Strikethrough `~~…~~`, Inline code `` `…` ``** share one unified contract. A *format token* is an opening delimiter immediately followed by content immediately followed by the matching closing delimiter. A target is a *whole token* when its first and last characters are exactly the command's open/close delimiters.

| # | State before the command | Behavior |
|---|---|---|
| T1 | Selection whose entire text is a whole token of this command | **Unwrap**: delete the delimiters; select the inner content; 1 undo step. An immediate second press now matches T2 → clean on→off round trip (mirrors the `dblclick` bold row in §7.1). |
| T2 | Any other non-empty selection whose text does **not** begin or end with this command's delimiter | **Wrap**: insert the delimiters around the selection; afterwards **expand the selection to include the new delimiters** so a second press matches T1 (guaranteed round trip, no nesting). 1 undo step. |
| T3 | Selection whose first *or* last character equals this command's delimiter (half-selected token, e.g. `**bold` or `old**`) | **No-op** + transient toast "Formatting markers are half-selected — include the whole word". Never produces doubled markers such as `****text****`. |
| T4 | Caret only (no selection), positioned inside a token of this command | **Unwrap the smallest whole token containing the caret that does not span a line break**; caret lands at the inner content's end. Whole-run unwrap for multi-word runs; split-run refinement is P1 (`[OPEN]` D14). |
| T5 | Caret in plain text, or inside a token of a *different* command | **Insert an empty pair** and place the caret between the delimiters (Bold → `****`, Italic → two asterisks, Strikethrough → `~~~~`, Inline code → two backticks); typing then produces formatted text. Pressing the **same command again while the pair is still empty** removes the pair (mode off before anything was typed). |
| T6 | Same command pressed twice rapidly (a true double-click) | Two executions per §6.12. Rows T1–T5 compose so the gesture either round-trips to the original text or is a no-op; each execution is its own undo step. |
| T7 | Wrap target spans two or more paragraphs | Allowed (valid Markdown) but the status bar hints "Prefer block formatting for multi-paragraph text". |
| T8 | Inline-code command on a **multi-line** selection | Inline code cannot span lines → command instead converts the selection to a **fenced code block** (preserves content); toast "Converted to code block". 1 undo step. |
| T9 | Selection is whitespace-only, or caret is at document start/end | Empty-selection rules T4/T5 apply. |
| T10 | Word double-clicked inside a token (§6.6) then formatted | The word is selected *with* its markers (existing rule), so the command sees a whole token → T1 unwrap. Consistent, no special case. |

**Active-state indicators (F109):** the Bold button renders active whenever the caret is inside a bold token; likewise Italic/Strikethrough/Inline-code. Indicators never trigger actions (G4). Indicator state follows the caret during selection changes.

**Empty-pair lifecycle (F108):** after T5, typing inserts content between the markers. Backspace while the pair is still empty removes the whole empty pair first (then normal backspace semantics resume). P1: an empty pair left by moving the caret away silently self-cleans (undo restores if the user cares).

**Block formatting — headings, blockquote, lists, code fences** operate on the **line containing the caret**, or on **every line the selection touches** (never joins or reflows lines beyond prefix/suffix changes). Toggles repeat to remove:

| Command | Applied form | Repeat behavior |
|---|---|---|
| Heading level 1–6 | Prefix each target line with `#` × level + space | Line already exactly at level N → demote to plain paragraph. Line at any other level → set to level N. Mixed selection → plain lines get level N, heading lines are set to N, all in one step. |
| Blockquote | Prefix each line with `> ` | All target lines already quoted → strip `> `. Mixed → quote only the unquoted lines. |
| Bullet list | Prefix each line with `- ` | All lines already bullets → strip; otherwise add. Tolerates existing `*` / `+` variants (converts them to `-` when stripping). |
| Numbered list | Prefix each line with `1. ` | Toggle like bullets; the renderer numbers sequentially (GFM) regardless of the literal digits. Existing `1.`/`2.` lines are normalized when stripping. |
| Task list | Prefix each line with `- [ ] ` | Toggle; converting a bullet to a task preserves the text; converting a task back drops the `[ ]`. |
| Fenced code | Wrap the paragraph(s) in ``` fences | Caret inside an existing fenced block → **remove that block's fences** (un-fence). Otherwise wrap. |

Empty lines inside a selection are skipped. Indentation is preserved; nested-list depth is raised only by Tab/indent (F47), never by these toggles. Mixed selections report "Formatted N of M lines" in the status bar. Block and inline commands are independent: Bold formats any selection even inside a heading line; a Heading command ignores the inline selection's character scope and operates on whole lines.

### 6.20 Tables — insert, extend, restructure (E24)

A *table block* is a contiguous run of lines forming a GFM table: one header row, one delimiter row (`| --- |`, optionally `:---`, `:---:`, `---:`), and zero or more body rows, with no blank line between rows. A *cell boundary* is an unescaped vertical bar. The editor never silently rewrites tables; every command below rewrites only its own table block and is exactly **one undo step** (§8.6). Raw source editing stays fully possible — commands are conveniences layered on §6.6 text editing.

**Inserting a table:**

| # | Event / Condition | Behavior |
|---|---|---|
| A1 | Toolbar / palette command "Insert table" | Opens the **grid picker**: a popover of empty cells up to 10 columns × 8 rows. |
| A2 | Hover over picker cells | Live preview highlight of R × C with a label "3 rows × 4 columns". Hover never inserts (G4). |
| A3 | Click a picker cell | Inserts the highlighted R × C GFM table at the caret: header row, delimiter row, R body rows, all columns padded; caret lands in the **first body cell**. 1 undo step. `Esc` or click-outside closes the picker with no insert. |
| A4 | Double-click a picker cell | One insert only — the picker closes on mousedown, so the second press of the double-click lands on the document, not the picker (G2, same pattern as §6.17). |
| A5 | No table tool used (raw typing) | User may type GFM directly; the preview renders it (§F20) and all commands below become available the moment the caret is inside a table block. |

**Extending & restructuring (available when the caret is in a table block, via right-click "Table" menu, palette, or the toolbar's table control):**

| # | Command / Condition | Behavior |
|---|---|---|
| B1 | Insert row below | Blank row with the block's column count appended immediately after the caret's row. If the caret is in the header row, the new row becomes the **first body row** (a row between header and delimiter would be invalid). |
| B2 | Insert row above | Blank row inserted immediately before the caret's row. If the caret is in the header, the new row becomes the new header and the old header drops to first body row. |
| B3 | Add column right | Append one empty cell (header ` `, delimiter `---`, body empty) to **every row** of the block. |
| B4 | Add column left | Prepend one empty cell to every row of the block. |
| B5 | Delete row | Removes the caret's row. Deleting the **only body row** leaves one empty body row so the table stays valid. |
| B6 | Delete column | Removes the caret's column from every row. Deleting the **last column** removes the whole table block (still 1 undo step). |
| B7 | Align column (left / center / right) | Rewrites the delimiter cell of the caret's column to `:---` / `:---:` / `---:`. |
| B8 | Move row up/down or column left/right | Reorders the block; the caret follows the moved row/column (P2 polish). |
| B9 | Result of every table command | A deterministic whole-block realign pass re-pads the pipes so columns line up in source. Preview refreshes on the §8.3 debounce. |
| B10 | Commands with a row/column selection (drag-selected rows) | Apply to every selected row/column at once. |

**Keyboard navigation inside a table (F99):**

| # | Event | Behavior |
|---|---|---|
| C1 | `Tab` in a table block, not in the last cell of the last row | Caret jumps just past the next cell boundary on the same line; at the line's last cell it moves to the first cell of the next row. Widths never change for navigation. |
| C2 | `Tab` in the **last cell of the last row** | Appends a new empty row and moves the caret into its first cell (fast data entry); the appended row is 1 undo step. |
| C3 | `Shift+Tab` | Inverse navigation (previous cell, or previous row's last cell). At the very first cell it is a no-op with a status hint. |
| C4 | `Enter` in a table row | Moves to the next row's first cell; at the last row, exits the table by starting a new paragraph below. (GFM cells cannot contain line breaks; multiline cells are authored in raw source, documented.) |
| C5 | Typing the vertical-bar character inside a table row | Auto-escapes it (backslash prefix) so the cell does not split in the renderer; the escape is part of the same undo step as the keystroke. |
| C6 | Pasting multi-line / TSV / CSV text while the caret is inside a table | P1 smart paste (§6.23 P4) turns it into table rows. P0 default: pastes as plain text with any vertical bars escaped (C5). |
| C7 | Caret leaves the table block (or the block was removed externally) | Table context ends; `Tab` resumes normal indent behavior (F47). |

**Copying tables:** right-clicking a rendered table in the preview offers **Copy as Markdown** and **Copy as CSV** (P1, F104). Selecting cells in the editor copies raw source text including the pipes (source semantics, §6.23 P10).

### 6.21 Images & media — insert, paste, copy (E25/E26)

Path model: a **document-relative** reference is used whenever the image sits under the markdown file's folder tree. When the app itself writes new image bytes it writes into a **media folder** — default `assets/` next to the document (settings: `assets/`, `media/`, custom, or "ask each time"; default per D11).

| # | Event / Condition | Behavior |
|---|---|---|
| M1 | Toolbar / palette "Insert image" | Native image multi-select dialog. For each chosen file inserts `![alt](path)` at the caret with `alt` = file stem pre-selected — typing overwrites it; `Enter` accepts and the caret moves after the closing parenthesis. The path is stored **relative to the md file** when the image is inside that file's tree; otherwise the media policy applies (outside-tree images are copied into the media folder when the setting allows, else an absolute path is inserted with a toast). One undo step covers all references from one dialog. |
| M2 | **Paste an image from the clipboard** (image present in clipboard, editor focused) | Saves the bytes to the media folder (`assets/«doc-stem»-YYYYMMDD-HHMMSS.png`), inserts `![alt](relpath)` with `alt` = file stem, and toasts "Image saved to assets/…". The buffer becomes dirty from the text insertion. The **file write is a documented side effect not covered by undo**: Undo removes the inserted reference but never deletes the file (cleanup is F106, P2). |
| M3 | Paste image while the buffer is **untitled** (no path yet) | Toast "Save the document first to attach images" and open the Save-As flow; on save the paste completes per M2. Cancel aborts the paste; the clipboard content is untouched. |
| M4 | Paste image while in **View mode** | No text target → nothing happens; the clipboard image is preserved for pasting elsewhere (no toast needed — nothing was attempted). |
| M5 | **Drag an image file from the OS** onto the editor | Inserts its reference at the drop point with the M1 path logic. Dropping an image onto the **preview** is a no-op with the toast "Drop images on the editor pane" (P1 refinement: insert at the preview's scroll anchor). Folder and non-image drops keep their §6.13 behavior. |
| M6 | Paste an image into the **preview** | No-op with a hint to switch to Edit mode (§6.5 toast pattern). |
| M7 | Right-click an image **reference in the editor** | Menu: Copy Path · Copy Relative Path · Copy as Markdown · Open in Viewer · Reveal in Folder · Copy Image (P1). |
| M8 | **Copy Image** (editor reference or preview image) | Reads the file bytes and places a real image (PNG/JPEG) on the clipboard using the native formats of §10. Never copies HTML or source text. |
| M9 | Right-click a **rendered image in the preview** | Menu (extends §6.7): Open in Viewer · Reveal in Folder · Copy Path · Copy Image (P1). |
| M10 | **Copy Path** / **Copy Relative Path** | Copy Path puts the absolute path on the clipboard; Copy Relative Path puts the md-relative path and is grayed out (with tooltip) when the image is outside the document's tree. |
| M11 | Reference to an image path that does **not exist** | Editor: fine (pure text). Preview: broken-image glyph + tooltip "Cannot load: path" (§6.7). The reference is never auto-deleted. |
| M12 | Document renamed or **Save As** to another folder with relative images | The app offers: "Copy referenced images to the new location?" — Yes copies them beside the new document and rewrites the references (1 undo step); No keeps references as-is. P1. |
| M13 | Filename collision for a newly written image (e.g. two pastes in the same second) | A numeric suffix is appended; an existing file is never overwritten silently. |
| M14 | Media folder not writable when saving a pasted image | Error toast; no partial state; the buffer is not dirtied. |
| M15 | Very large image pasted (over 20 MB or 12000 px) | Saved as-is but flagged in the status bar; the preview downscales for display without touching the file. Decode budget follows §9. |

### 6.22 Links — insert, edit, remove, copy (E25)

| # | Event / Condition | Behavior |
|---|---|---|
| L1 | **Insert link** (`Ctrl/Cmd+K`) with a non-empty selection | Wraps the selection: `[«selection»](url)` and **selects the url part for overwrite**; typing replaces it. `Enter` or `Tab` accepts and the caret moves after the closing parenthesis. `Esc` while the url is selected cancels the insert (which is 1 undo step regardless). |
| L2 | Insert link with caret only (no selection) | Inserts `[text](url)` with **`text` selected as a placeholder**; typing replaces the text, `Tab` moves the selection to the url part, a second `Enter`/`Tab` accepts. Fully keyboard-operable (§11). |
| L3 | **Edit an existing link** (caret inside the link, or right-click → Edit Link) | Selects the url portion for overwrite, then L1 semantics continue. Right-click additionally offers "Paste URL from clipboard" into the url field without discarding the current one (P1). |
| L4 | **Remove link** (context menu, or caret inside a link + command) | Removes the `(url)` part and the brackets, leaving the plain text; 1 undo step. |
| L5 | Link button pressed twice rapidly | First execution starts the L1/L2 flow; the second press while the inline flow is open is **ignored** (the flow is inline-modal). No double insertion ever. |
| L6 | "Insert link to file…" (local Markdown file) | Native picker restricted to Markdown; inserts a md-relative path using the M1 path logic. Preview click behavior per §6.7. |
| L7 | Right-click a link **in the editor** | Menu: Edit Link · Remove Link · Copy Link Address (the URL, not the text) · Copy as Markdown · Open Link. |
| L8 | Insert/edit/remove link in **View mode** | Toolbar hidden (E23 hidden in View, §6.5); no link text manipulation in a read-only preview. |
| L9 | Link left empty `[](url)` after the flow | Kept as typed; GFM renders the URL as the link text. No auto-repair. |

P1 refinement (F107): an inline popover at the caret with Text / URL / Title fields replaces the sequential placeholder flow; `Enter` = OK, `Esc` = cancel, and both emit the same Markdown. Until then the placeholder flow above is the default.

### 6.23 Clipboard — paste semantics & Markdown-aware copy (E26)

| # | Event / Condition | Behavior |
|---|---|---|
| P1 | Paste with **plain text** in the clipboard | Inserts exactly that text (line breaks preserved). Plain text is never auto-converted. |
| P2 | Paste with **rich/HTML** content (web page, Word, spreadsheet) | Default: **plain-text paste** — only the text content is inserted (predictable). Setting "Smart paste (HTML → Markdown)" is OFF by default (`[OPEN]` D13). |
| P3 | Smart paste ON: HTML contains headings, strong/em, lists, code | Converts to equivalent Markdown (GFM rules) preserving order, then inserts. The whole paste is 1 undo step. |
| P4 | Smart paste ON (or "Paste as table" action offered, P1): clipboard holds a table (HTML `<table>` or TSV/CSV text) | Inserts a GFM table block (first row becomes the header, HTML alignment is honored). P0: TSV/CSV pasted as text; the toast offers "Paste as table". |
| P5 | Paste with an **image** in the clipboard | Image path per §6.21 M2 — raw image bytes are never pasted into the text. |
| P6 | Paste while a **modal dialog** is open | Blocked by the modal lock (§8.1). |
| P7 | Paste target is the Find & Replace input (§6.16) | Text-only paste into the field. |
| P8 | **Copy from the preview** (a selection of rendered text) | Copies the **Markdown source** that produced the selection (F104) — never HTML. Code blocks copy as fenced Markdown, tables as GFM, links as `[text](url)`. |
| P9 | Copy from the preview where the selection includes an **image** | Copies the image *reference* as Markdown; use Copy Image (M8) when image bytes are wanted. |
| P10 | Cut / Copy from the **editor** | Standard source-text semantics — raw Markdown including delimiters and vertical bars. |
| P11 | Rich text (RTF/HTML) is **never** placed on the clipboard by the app | Guarantees Markdown round-trips; parity notes in §10. |
| P12 | Paste size guard | Text paste over 5 MB triggers a confirm dialog (prevents accidental mega-inserts); image pastes are guarded by M15. |

Acceptance note: the §7.1 stress-table rows added above exercise the double-click paths of these contracts; defaults tracked in D11–D15 stay open until Phases 3–4.
---

## 7. Failure & edge-case scenarios (state × event)

### 7.1 Double-click / repeated-click stress table (G-canonical cases)

| Situation | Consequence (guaranteed) |
|---|---|
| `dblclick` the tab **X** | 1 close flow (dialog or close). Never closes behind dialog. |
| `dblclick` window **X** with dirty buffer | 1 dialog. Never quit-while-dirty. |
| `dblclick` file row | 1 open. Single-click preview defer absorbs the first click. |
| 10× rapid `Ctrl+S` | Coalesced writes: saving flag drops repeats; final state on disk = last content. |
| 2× `Ctrl+W` rapidly | First closes/guards; second acts on the *next* active tab only after first resolved. Never closes 2 tabs in one gesture. |
| `dblclick` bold toolbar button | Toggles on then off (two real single clicks — intentional, both recorded in undo). |
| Double-click a disabled button | Disabled → no events fire. |
| Double-click during modal dialog | Only dialog receives input; underlying gestures discarded. |
| Click two different tab X's ultra-fast (before first dialog resolves) | Second click targets second tab's guard; two dialogs never stack — second waits (§8.1 serialization). |
| Drag that ends exactly on a double-click time window | Movement >4 px resets the click counter (G1) → it is a drag, not a click. |
| Click + keyboard shortcut at same instant | Keyboard wins; click discarded if it would conflict (e.g., Ctrl+W while clicking X). |
| `dblclick` Bold / Italic on a selected word | Toggles on then off (two intentional single executions per §6.12); per §6.19 T1/T2 the text ends unchanged; 2 undo steps. |
| `dblclick` an Insert-table picker cell | One insert only — the picker closes on mousedown so the second click falls through (§6.20 A4). |
| 2× rapid `Ctrl/Cmd+K` (insert link) | One link flow; the second keypress while the inline flow is open is ignored (§6.22 L5). |
| Paste image then immediate Undo | Inserted reference removed; the **saved image file remains on disk** (documented side effect, §6.21 M2 / F106). |
| `dblclick` a table "delete row" command | Two real deletes: removes the caret row then the row that followed — intentional per §6.12, both undoable. |

### 7.2 Data-loss prevention matrix (close / reload / replace / quit)

| Event | Buffer clean | Buffer dirty · autosave off | Buffer dirty · autosave on |
|---|---|---|---|
| Tab close | Close now | Save/Discard/Cancel | Save (autosave flush) → close |
| Window close / quit | Quit now | Dialog per dirty buffer (one combined list) | Flush all autosaves → quit |
| Open file in same tab | Replace now | Dialog first (Cancel aborts) | Autosave flush → replace |
| File externally changed (focus returns) | Auto-reload (or silent) | Dialog: Reload / Keep mine (save over) / Compare (P2) / Cancel | Autosave-flush first, then external-reload rules against the flushed disk state |
| File deleted on disk (still open) | Status: "File missing — Save re-creates / Close" | Same + "Save re-creates file" is offered and safe | Same |
| File became read-only | Edit blocked? No: edits allowed in buffer; Save shows "Permission denied" + offers Save As; dirty dot persists | same | Autosave fails → toast, recovery store keeps content |
| App crash | — | — | Recovery banner on next start with full unsaved text (§8.7) |
| Reopen same file (2nd instance) | Second instance opens its own buffer (P2 lock warning) | — | — |

---

## 8. State machine & timing contracts

### 8.1 Single-modal-instance rule
At most **one** modal dialog exists per app. A guarded command that arrives while a dialog is open is **discarded** (never queued, never auto-executed). Rationale: queued closes are how apps eat documents. Only exception: an *explicit second user action after the dialog resolves* re-runs the command. **Multi-buffer close** (Close All / window X with several dirty buffers) shows **one dialog listing all dirty files** with per-file Save/Discard choice + global Save All / Discard All / Cancel.

### 8.2 Preview scroll anchoring
On re-render after an edit, preview keeps the *first visible element* stable if it was not the edited element; if the edit is in view, the caret's block is kept visible instead. No jarring jumps for edits below the fold.

### 8.3 Render debounce
Edit → preview pipeline: text events coalesce; render fires 150 ms after the last edit, or immediately (flush) when: switching to View mode, printing/exporting, or a save completes. Render is skipped entirely when the preview is not visible (Edit mode) — costs nothing until Split/View.

### 8.4 External-change watcher
A file watcher (per open file) detects external modification/deletion. State is only acted upon when the window **gains focus** (avoids dialog storms while user works). See matrix §7.2. On **Save**, RustDownViewer does **not** overwrite blindly if the file changed on disk since load/save: it shows Reload/Overwrite/Cancel (P0: last-writer-wins is dangerous).

### 8.5 Autosave (P1, `[OPEN]`: default on/off)
When on: every **3 s** of inactivity after an edit (and at blur/quit), buffer is saved **if it has a path**; untitled buffers go to the recovery store only. Autosave never interrupts typing (main-thread write is atomic and async where the engine allows). Autosave failures surface once (toast), not repeatedly; retry on next cycle.

### 8.6 Undo semantics
Undo stack is per buffer and **not cleared by Save/Autosave** (matches modern editors; accidental-save-then-undo must work). Undo history is truncated only when the buffer is closed (after guard) or its content is replaced by an external reload (which itself is undoable? No — reload resets the stack with one "reload" marker; Undo cannot resurrect pre-reload text; the external-change dialog warned the user).
Replace-all, mode switches, saves: do not fragment the undo stack (replace-all = one step).

### 8.7 Crash recovery store
Every dirty buffer is mirrored (debounced 500 ms) to a per-app recovery directory (OS temp/config). On startup, if recovery files exist: show **Recovery banner** listing "Recovered changes in «file» (time)" with buttons: Recover / Discard / Show All. Recover = open buffer with changes (path restored if original still exists; else Save As). Banner state is itself persisted so the choice isn't re-asked. Clean exit deletes recovery mirrors.

### 8.8 Read-only & permission handling
Detecting read-only at open: banner "File is read-only — you can edit but saving requires Save As…". Edits still allowed in the buffer (dirty dot shows). Save attempt failing → dialog with **Retry / Save As… / Discard changes**. Permission loss mid-session → same path without crashing.

---

## 9. Performance & input boundaries (behavior under extreme conditions)

| Condition | Behavior (contract) |
|---|---|
| File **> 10 MB** or **> ~200k lines** at open (P0 estimate) | Opens fast (engine must not block >300 ms) with a toast "Large file — preview features limited". |
| Large file + **preview rendering** | Preview switches to **virtualized** rendering (only visible blocks rendered). Editor uses lazy line layout. |
| User types in a 50 MB file | Acceptable latency budget: keystroke-to-glyph < 50 ms (P95); render debounce work happens off the UI thread. |
| Image that is 100 MB | Decoded lazily on visibility with dimension cap (e.g., 4096 px); never blocks UI. |
| 1000s of matches in Find | Highlight pass throttled (yield every 8 ms); match count shows "1000+" until pass completes. |
| Opening 200 files at once (multi-select) | Tabs open with a progress affordance; UI never freezes; files load in priority order. |
| Network drive latency / unresponsive file | I/O runs off main thread; if a read/write exceeds 5 s, show "Waiting for disk…" with Cancel; app remains responsive. |
| Out of memory risk on pathological file | Open is size-guarded by default (warn > 250 MB with "Open anyway" requiring explicit click). |

---

## 10. Cross-platform deltas (behaviors that differ per OS)

| Aspect | Windows | macOS | Linux |
|---|---|---|---|
| Modifier key | Ctrl | Cmd | Ctrl |
| Primary close | X top-right | Red light top-left | X top-right |
| Exit shortcut | Alt+F4 / Ctrl+Shift+W | Cmd+Q | Ctrl+Shift+W / Alt+F4 |
| Menu bar | In-window | **Global system menu bar** (app menu must move) | In-window |
| Title bar | OS-drawn (default) | OS-drawn (unified toolbar) | OS-drawn per DE; CSD variants honored |
| File association | Registry (installer) | Info.plist / UTI | `.desktop` MimeType |
| Open-with / recent | JUMP LIST (P2) | Recent Documents | Recent Files (XDG) |
| System theme signal | AppsKey light/dark | AppleInterfaceStyle + per-app override | DE-specific (GTK/Qt portals) |
| Font rendering / default UI font | Segoe UI | SF Pro | Distro default |
| Paths & casing | Case-insensitive; `\` | Case-insensitive-ish; `/` | Case-sensitive; `/` |
| IME | TSF | InputMethodKit | IBus/Fcitx |
| Clipboard image/RTF | Rich formats where sensible | NSPasteboard | X11/Wayland (wl-clipboard parity caveat documented) |
| Printing | Via system | Via system | Via system; headless-safe fallback |
| Drag & drop | OLE | NSDragging | XDND / Wayland DnD (portal) |
| Global rule | **Behavior contract identical on all three; only the *mechanism* differs.** Any place a platform cannot deliver a contract behavior → documented in-app notice, never a silent difference. |

---

## 11. Keyboard-only acceptance scenarios (a11y, P1)

Every §6 mouse behavior has an equivalent path. Spot-audit checks:
1. Close dirty tab with only keyboard → guard dialog reachable, default Save, `Enter` saves and closes.
2. Close window with 3 dirty buffers → one combined list dialog, per-file choices via arrow keys.
3. File tree: create/rename/delete via keyboard with guards; preview follows selection.
4. Command palette fully operable with keyboard; every menu item listed.
5. Dialog destructive button unreachable by accidental `Enter`.
6. Full-screen toggle (F11) restores previous window bounds.
7. Focus never trapped: Tab order = panes → toolbar → status → dialogs; visible focus ring on all platforms.
8. Contrast: dark/light themes meet WCAG AA for text; high-contrast theme (P1).

---

## 12. Open decisions `[OPEN]` (gate before Phase 1)

| # | Decision | Options & lean |
|---|---|---|
| D1 | GUI framework | **Tauri v2** (webview UI + Rust core) vs **egui/eframe** (pure Rust, no webview). Lean: Tauri v2 for editor richness (CodeMirror-class editing, mature IME/accessibility) with Rust doing parsing/saving; egui if "one static binary, no webview" wins. |
| D2 | Markdown engine | `pulldown-cmark` (fast, commonmark/GFM) vs `comrak` (GFM + tables/extensions out of the box). Lean: comrak or pulldown-cmark + table extension; verify against GFM spec corpus in Phase 2. |
| D3 | Syntax highlighting | `syntect` (Sublime syntaxes, offline) — near-certain yes. |
| D4 | Editor core | Web: CodeMirror 6 vs Monaco. Native: custom rope (`ropey`) + custom view layer. Depends on D1. |
| D5 | Autosave default | on/off default (`[OPEN]`; lean: default **on** with recovery store guarantees, matching §7.2). |
| D6 | Hard-tab policy | lean: spaces default; respect `.editorconfig` (P1). |
| D7 | Multi-instance file locking | P2; lean: advisory warning only. |
| D8 | Export/print engine | System print dialog vs bundled (wkhtmltopdf-like) engine; lean system print for P1. |
| D9 | Quad-click & 4th-level gestures | Support paragraph select at 4 clicks, nothing beyond. |
| D10 | Word double-click inside markdown markers | select inner word vs whole token `[OPEN]` (lean: whole token first, inner refinement P1). |
| D11 | Pasted / dropped image storage | Media-folder policy: default `assets/` next to the document vs ask-each-time vs global app media dir. Lean: `assets/` default + per-install setting; Save-As relocation offer (§6.21 M12). |
| D12 | Table editing model | Source-assisted commands (§6.20) vs WYSIWYG grid overlay inside the editor. Lean: source-assisted for P0/P1; grid overlay P2 only if the editor engine makes it cheap. |
| D13 | Smart-paste default (HTML/rich/TSV → Markdown) | Auto-conversion ON vs OFF. Lean: **OFF** (plain-text paste) for predictability; enable only after conversion is corpus-tested (§6.23). |
| D14 | Unwrap granularity inside formatted runs | Whole-token unwrap (§6.19 T4) vs split-run unwrap preserving surrounding formatting. Lean: whole-token for P0; split-run refinement P1. |
| D15 | Strikethrough / inline-code / heading keybindings | Reserve keys that do not collide with OS/DE combos (Ctrl+Shift+C = terminal copy on Linux). Lean: Ctrl/Cmd+Shift+X = strikethrough; Ctrl/Cmd+Shift+` = inline code; headings via toolbar/menu (palette later). `[OPEN]` until the Phase 3 keymap audit. |

**Gate rule:** D1–D4 are resolved in Phase 1 with a decision record (ADR) before any UI code; D5 defaults applied and revisitable. D11–D15 gate their respective Phase 3 (editor/formatting/tables) and Phase 4 (clipboard/media) work and must be resolved there with the same ADR discipline.

---

## 13. Milestone "Definition of Done" hooks

- **P0 (v0.1.0)** passes: §7.1 stress table automated (UI event-script tests), §7.2 matrix manual test sheet signed off on **all three OSes**, §8 timing contracts instrumented, no known data-loss path (Section 7 matrix fully green).
- Each §6 element table is the acceptance checklist for that element's implementation ticket.

---

*End of specification. Every behavior above is intended to be testable: given a UI state + an input sequence → one asserted outcome.*
