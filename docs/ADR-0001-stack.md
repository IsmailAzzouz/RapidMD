# ADR-0001 — GUI / parsing / highlighting stack (resolves spec decisions D1–D4)

**Date:** 2026-09-09
**Status:** Accepted

## Decision

| Spec decision | Choice | Rationale |
|---|---|---|
| D1 GUI framework | **egui / eframe** (pure Rust, immediate-mode, renders via glow/WGPU) | Single native binary, **no webview, no Node toolchain**, fully cross-platform (Win/macOS/Linux), trivially runnable with `cargo run`. "Native" in the strictest sense: all pixels drawn by Rust. Tauri rejected: requires system webview + JS toolchain, violating the "no cheat, native, cargo run" constraint. |
| D2 Markdown engine | **pulldown-cmark 0.12** (CommonMark + GFM tables/strikethrough/tasklists/footnotes via options) | Fast, correct, event-stream API that maps 1:1 onto an egui block renderer. |
| D3 Syntax highlighting | **syntect** (Sublime `.sublime-syntax` grammars, ~200 bundled) for code fences; hand-rolled markdown-aware highlighter for the editor source | syntect = offline, high quality; editor highlighting kept dependency-free and unit-testable. |
| D4 Editor core | Native egui `TextEdit` (multiline, undoable) on a `ropey`-free in-memory `String` buffer owned by the app | egui TextEdit provides IME, clipboard, selection for free; buffer model stays plain text with snapshot undo (testable, no engine lock-in). |

## Design invariants resulting from this decision

1. One process, one `eframe::App`, one `Vec<Buffer>` = the document model. UI is a pure projection of buffer state each frame (spec §1 principle 1: *one buffer, one truth*).
2. No data loss: all writes are atomic (temp file + rename, same volume); dirty-guard modals are app-modal overlays rendered by egui (single instance guaranteed by an enum, spec §8.1).
3. Preview is generated from buffer text via pulldown-cmark on a change-detected cache; rendering is skipped when no preview is visible (spec §8.3).
4. Cross-platform by construction: we use only eframe/egui abstractions + `rfd` (native dialogs) + `open` (OS open); no platform-specific code paths except OS-drawn window chrome which egui defers to the OS.

## Deferred (explicitly, not silently)

P1/P2 features that would need a full-time editor engine or web tech (multi-cursor, WYSIWYG table grid, Mermaid, remote image fetching, spellcheck) are marked `[deferred: P1/P2]` in the coverage matrix and never silently claimed. See `docs/SPEC_COVERAGE.md` at completion.
