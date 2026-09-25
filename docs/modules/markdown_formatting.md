# Markdown Engine & Formatting Modules Technical Documentation

This document provides comprehensive technical documentation for the core markdown parsing, editing, syntax highlighting, HTML compatibility, and text utility modules in RustDownViewer:
- [`src/md.rs`](#mdrs---intermediate-rich-text-model--parser) — Intermediate rich-text model and parsing engine.
- [`src/format.rs`](#formatrs---markdown-editing-operations) — Buffer editing operations and smart formatting.
- [`src/highlight.rs`](#highlightrs---markdown-aware-editor-tokenization) — Line-oriented syntax highlighting tokenizer.
- [`src/html.rs`](#htmlrs---zero-dependency-html-compatibility-parser) — Zero-dependency HTML parser, entity decoder, and block/inline converter.
- [`src/text_utils.rs`](#text_utilsrs---pure-text-helpers--line-arithmetic) — Byte/character offset conversion and line arithmetic helpers.

---

## Architecture & Module Interactions

The markdown subsystem is designed around **rendering-agnostic separation of concerns**:
1. **Parsing (`md.rs`)**: Consumes raw markdown text using `pulldown-cmark`, applying GFM extensions, footnotes, tables, and task lists, and translating parser events into an immutable, owned abstract syntax tree (`Doc`, `Block`, `Span`).
2. **HTML Interoperability (`html.rs`)**: Provides robust zero-dependency parsing of embedded HTML blocks and inline tags (such as styling tags, links, images, and center alignments), bridging HTML markup into the rich-text AST.
3. **Editing & Formatting (`format.rs`)**: Implements pure transformation functions over buffer text and byte selections, providing single-step undo primitives, inline delimiter toggling (spec §6.19), block formatting, and Smart Enter handling for lists.
4. **Editor Syntax Highlighting (`highlight.rs`)**: Offers a fast, line-oriented tokenizer with persistent fenced-code state for syntax coloring in text editors.
5. **Text Arithmetic (`text_utils.rs`)**: Bridges egui's character-based cursor model with Rust's UTF-8 byte-indexed `String` slices, providing line bounds, column tracking, and word-range selection.

---

## `md.rs` — Intermediate Rich-Text Model & Parser

`src/md.rs` defines the intermediate AST model representing parsed markdown documents independently of any GUI renderer.

### Core Data Models

#### `Style` & `Span`
Inline formatting spans represent styled text fragments within blocks:
```rust
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}
```
`Span` enum variants:
- `Text { text: String, style: Style }`: Plain text with style flags.
- `Code { text: String, style: Style }`: Monospaced inline code.
- `Link { url: String, title: Option<String>, inner: Vec<Span> }`: Hyperlink with styled inner spans.
- `Image { url: String, alt: String, title: Option<String>, width: Option<f32>, height: Option<f32> }`: Embedded image with optional dimensions.
- `LineBreak`: Hard line break within a paragraph.
- `FootnoteRef { label: String }`: Footnote reference `[^label]`.
- `Math { text: String }`: Mathematical expression (`$...$`).

#### `Block` & `Doc`
Block-level elements are structured via the `Block` enum:
- `Heading { level: u8, spans: Vec<Span> }`: ATX headings (levels 1–6).
- `Para(Vec<Span>)`: Paragraphs.
- `BlockQuote { kind: QuoteKind, items: Vec<Block> }`: Blockquotes supporting GFM callouts (`QuoteKind`: `Note`, `Tip`, `Important`, `Warning`, `Caution`, `Plain`).
- `List { ordered: bool, items: Vec<ListItem> }`: Ordered and unordered lists (including task list items with `checked: Option<bool>`).
- `CodeBlock { lang: String, code: String }`: Fenced or indented code blocks.
- `Table(Table)`: Tables with column alignments (`ColAlign`), header cells, and row data.
- `Rule`: Horizontal rule (`---`).
- `Html(String)`: Raw HTML blocks.
- `FootnoteDef { label: String, blocks: Vec<Block> }`: Footnote definitions.
- `Center(Vec<Block>)`: Centered content blocks.

The top-level `Doc` structure bundles blocks, a Table of Contents (`toc: Vec<(u8, String, String)>` containing level, plain title, and GitHub-style slug), and extracted footnotes.

### Parsing Pipeline
1. **Options Configuration**: Enables GFM extensions (`ENABLE_TABLES`, `ENABLE_FOOTNOTES`, `ENABLE_STRIKETHROUGH`, `ENABLE_TASKLISTS`, `ENABLE_HEADING_ATTRIBUTES`, `ENABLE_GFM`).
2. **Owning Conversion (`own_event`)**: Converts `pulldown_cmark` borrowed `CowStr` events into `'static` owned strings, allowing the resulting `Doc` to outlive the input buffer.
3. **Block & Span Parsing**: Recursive block parsers (`parse_blocks`) and inline parsers (`parse_spans_styled`) build AST structures, correctly handling tight/loose lists, inline HTML tags, and nested formatting.
4. **TOC & Slugification**: `slugify` generates GitHub-compatible URL anchors (converting non-alphanumeric characters to dashes and handling duplicate headings), while `spans_plain` extracts plain text from inline spans.

---

## `format.rs` — Markdown Editing Operations

`src/format.rs` implements pure editing functions for markdown buffers (spec §6.19–§6.22). Every operation takes buffer text and a byte range selection, returning an `Out` enum:
- `Out::Applied(Op)`: Successful operation with `new_text` and updated `selection` range.
- `Out::Noop(&'static str)`: Refusal message (e.g. half-selected delimiters) surfaced as a transient toast.

### Key Editing Features
- **Inline Delimiter Toggling (`toggle_wrap`)**: Implements spec rules T1–T5 for bold, italic, strikethrough, and code wrapping:
  - T1: Selection covers a whole `open...close` token -> unwrap.
  - T2: Plain wrap -> wraps selection and selects the whole token for easy round-trip toggling.
  - T3: Half-selected markers -> refuses to run (`Noop`), preventing malformed markers.
  - T4: Caret inside token -> unwraps token.
  - T5: Caret on plain text -> inserts empty marker pair with caret in between.
- **Block Formatting**:
  - `toggle_heading`: Toggles ATX heading levels (1–6) across touched lines; repeating the same level demotes to a plain paragraph.
  - `toggle_blockquote`: Adds or removes `> ` prefixes.
  - `toggle_bullet_list` & `toggle_numbered_list`: Toggles bullet (`- `, `* `, `+ `) or numbered (`1. `) prefixes.
  - `toggle_task_list`: Toggles task list items (`- [ ] ` / `- [x] `).
- **Smart Enter (`smart_enter`)**:
  - Automatically continues bullet, numbered, and task lists on pressing Enter.
  - Incrementing sequence numbers for numbered lists.
  - Pressing Enter on an empty list item automatically strips the marker, gracefully exiting the list.

---

## `highlight.rs` — Markdown-Aware Editor Tokenization

`src/highlight.rs` provides a line-oriented syntax tokenizer (`tokenize`) for code editors, producing non-overlapping byte ranges paired with token categories (`Tk`):

### Token Categories (`Tk`)
`Plain`, `Heading`, `Strong`, `Emphasis`, `Strike`, `Code`, `LinkText`, `LinkUrl`, `ListMarker`, `Quote`, `Task`, `Hr`, `Html`, `Fence`, `CodeText`.

### Tokenization Strategy
- **Fenced Code Tracking**: Tracks persistent state (`in_fence`) across lines when encountering code fences (`````` or `~~~`), styling fences as `Tk::Fence` and inner lines as `Tk::CodeText`.
- **Line-by-Line Scanning**:
  - Checks for horizontal rules (`is_hr`).
  - Detects task and list markers (`task_or_list_marker`, `list_marker`).
  - Identifies blockquote prefixes (`> `).
  - Recognizes ATX heading lines.
  - Scans inline emphasis, code, and links within remaining line text.

---

## `html.rs` — Zero-Dependency HTML Compatibility Parser

`src/html.rs` provides zero-dependency parsing of HTML blocks and inline tags to support hybrid markdown/HTML documents.

### Key Capabilities
- **Entity Decoding (`decode_entities`)**: Decodes standard HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;`) as well as decimal and hexadecimal character references (`&#33;`, `&#x20;`).
- **Dimension Parsing (`parse_dimension`)**: Parses CSS dimension strings like `"96"` or `"96px"` into `f32` values for image sizing.
- **HTML Tokenization (`tokenize_html` & `extract_attrs`)**: Tokenizes HTML strings into `HtmlToken` variants (`TagOpen`, `TagClose`, `Text`), extracting attributes robustly even with unquoted or single/double-quoted values.
- **Inline Token Conversion (`parse_inline_tokens`)**: Converts HTML inline tags (`<b>`, `<strong>`, `<i>`, `<em>`, `<s>`, `<del>`, `<code>`, `<kbd>`, `<a>`, `<br>`, `<img>`) into rich-text `Span` elements.
- **Block & Center Alignment (`parse_html_block_content`)**: Parses HTML blocks, recognizing center alignments (`<center>`, `<div align="center">`, style `text-align: center`) and mapping them to `HtmlBlockAction` triggers (`OpenCenter`, `CloseCenter`, `Block`).

---

## `text_utils.rs` — Pure Text Helpers & Line Arithmetic

`src/text_utils.rs` bridges egui's character-based cursor indexing with Rust's byte-indexed `String` slicing.

### Utility Functions
- **Offset Conversions**:
  - `byte_to_char(text, byte)`: Converts a UTF-8 byte offset to a character offset.
  - `char_to_byte(text, ch)`: Converts a character offset to a byte offset (clamped to buffer length).
- **Line Arithmetic**:
  - `line_start(text, byte)` / `line_end(text, byte)`: Finds line start/end boundaries.
  - `line_bounds(text, line_idx)`: Returns `(start, end_excl, end_incl)` for 0-based line indices.
  - `line_count(text)`: Total number of logical lines.
  - `line_column(text, byte)`: 0-based character column within a line.
  - `line_index(text, byte)`: 0-based line index containing a byte offset.
- **Word Navigation**:
  - `word_range_at(text, byte)`: Computes the `(start, end)` byte range of the word or non-whitespace token containing the caret.
