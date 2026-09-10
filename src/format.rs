//! Markdown *editing* operations (spec §6.19–§6.22, F94–F100).
//!
//! Everything here is a pure function over the buffer text + a byte-based
//! selection, returning the new text plus a new selection/caret. The UI layer
//! snapshots an undo point before applying an `Op` (single-step undo).

use crate::text_utils::{line_bounds, line_count, line_index};

/// Result of a successfully applied operation.
pub struct Op {
    pub new_text: String,
    /// Byte range to select afterwards (sorted).
    pub selection: std::ops::Range<usize>,
}

/// An op may refuse to run (spec T3: half-selected delimiters).
pub enum Out {
    Applied(Op),
    /// No change; the message is shown as a transient toast.
    Noop(&'static str),
}

impl Out {
    pub fn unwrap_op(self) -> Op {
        match self {
            Out::Applied(op) => op,
            Out::Noop(msg) => panic!("unexpected noop: {}", msg),
        }
    }
}

fn replace_range(text: &str, range: std::ops::Range<usize>, with: &str) -> String {
    let mut s = String::with_capacity(text.len() + with.len());
    s.push_str(&text[..range.start]);
    s.push_str(with);
    s.push_str(&text[range.end..]);
    s
}

fn apply(text: &str, range: std::ops::Range<usize>, with: &str, new_sel: std::ops::Range<usize>) -> Out {
    let new_text = replace_range(text, range, with);
    let sel_end = new_sel.end.min(new_text.len());
    let sel_start = new_sel.start.min(sel_end);
    Out::Applied(Op { new_text, selection: sel_start..sel_end })
}

// ---------------------------------------------------------------------------
// Inline wrappers (spec §6.19 T1–T5)
// ---------------------------------------------------------------------------

/// Does `sel` cover a whole `open…close` token?
fn is_whole_token(text: &str, sel: &std::ops::Range<usize>, open: &str, close: &str) -> bool {
    if sel.len() < open.len() + close.len() {
        return false;
    }
    text.get(sel.start..sel.start + open.len()) == Some(open)
        && text.get(sel.end - close.len()..sel.end) == Some(close)
}

/// Find `(token_start, token_end)` enclosing `byte` for `open/close`, if any.
fn enclosing_token(text: &str, byte: usize, open: &str, close: &str) -> Option<(usize, usize)> {
    let line_start = text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let mut search = line_start;
    while search < byte {
        if text[search..].starts_with(open) && !is_escaped(text, search) {
            let content_end = search + open.len();
            if content_end > byte {
                return None; // later than caret — caret inside open delim?
            }
            if let Some(rel) = text[content_end..].find(close) {
                let close_at = content_end + rel;
                if close_at >= byte && !is_escaped(text, close_at) {
                    let token_end = close_at + close.len();
                    // Only accept if there is no later close between content and byte
                    let within = &text[content_end..close_at];
                    let _ = within;
                    return Some((search, token_end));
                }
            }
        }
        search += text[search..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    None
}

fn is_escaped(text: &str, at: usize) -> bool {
    let mut backslashes = 0usize;
    let before = &text[..at];
    for c in before.chars().rev() {
        if c == '\\' {
            backslashes += 1;
        } else {
            break;
        }
    }
    backslashes % 2 == 1
}

pub fn toggle_wrap(text: &str, sel: &std::ops::Range<usize>, open: &str, close: &str) -> Out {
    debug_assert!(sel.start <= sel.end && sel.end <= text.len());

    if sel.is_empty() {
        // T4: caret inside an existing token of this command → unwrap it
        // (remove the delimiters, keep the inner content).
        let byte = sel.start;
        if let Some((ts, te)) = enclosing_token(text, byte, open, close) {
            let inner = (ts + open.len())..(te - close.len());
            let inner_text = &text[inner.clone()];
            let sel_start = ts;
            return apply(text, ts..te, inner_text, sel_start..(sel_start + inner.len()));
        }
        // T5: caret anywhere else → insert an empty pair, caret between markers.
        let caret = byte + open.len();
        return apply(text, byte..byte, &format!("{}{}", open, close), caret..caret);
    }

    // T1: the selection covers a whole `open…close` token → unwrap.
    if is_whole_token(text, sel, open, close) {
        let inner = (sel.start + open.len())..(sel.end - close.len());
        let inner_text = &text[inner.clone()];
        let sel_start = sel.start;
        return apply(text, sel.clone(), inner_text, sel_start..(sel_start + inner.len()));
    }

    // The selection is the exact inner content of a token (delimiters adjacent).
    let pre = sel.start.checked_sub(open.len()).and_then(|s| text.get(s..sel.start));
    let post = text.get(sel.end..sel.end + close.len());
    if pre == Some(open) && post == Some(close) {
        let ts = sel.start - open.len();
        let te = sel.end + close.len();
        let content = &text[sel.clone()];
        let sel_start = ts;
        return apply(text, ts..te, content, sel_start..(sel_start + sel.len()));
    }

    // T3: half-selected delimiter → refuse (never double the markers).
    let sel_text = &text[sel.start..sel.end];
    let starts_with_open = text.get(sel.start..sel.start + open.len()) == Some(open);
    let ends_with_close = text.get(sel.end - close.len()..sel.end) == Some(close);
    if (starts_with_open || ends_with_close) && sel_text != open && sel_text != close {
        return Out::Noop("Formatting markers are half-selected — include the whole word");
    }

    // T2: plain wrap; select the whole token so the next press round-trips.
    toggle_wrap_range(text, sel.clone(), open, close)
}

fn toggle_wrap_range(text: &str, sel: std::ops::Range<usize>, open: &str, close: &str) -> Out {
    let wrapped = format!("{}{}{}", open, &text[sel.clone()], close);
    // Select the *whole token* afterwards so the next press round-trips (T2).
    let token_sel = sel.start..(sel.end + open.len() + close.len());
    apply(text, sel, &wrapped, token_sel)
}

/// Largest char boundary `<= at`, clamped to `text.len()` (never panics on
/// stale or mid-character offsets).
fn floor_boundary(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Lines touched by `sel` (or the caret's line when empty).
fn touched_line_bounds(text: &str, sel: &std::ops::Range<usize>) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }
    let start = floor_boundary(text, sel.start);
    let end = floor_boundary(text, sel.end).max(start);
    let first_line = text[..start].bytes().filter(|&b| b == b'\n').count();
    let mut last_line = text[..end].bytes().filter(|&b| b == b'\n').count();
    // A caret sitting on the line terminator belongs to the line before it, and
    // probing one byte past the buffer used to panic (H1/H2 "break the app").
    if end > start && text.get(end.saturating_sub(1)..end) == Some("\n") {
        last_line = last_line.saturating_sub(1);
    }
    (first_line, last_line.max(first_line))
}

// ---------------------------------------------------------------------------
// Block formatting (headings, quotes, lists)
// ---------------------------------------------------------------------------

fn heading_prefix(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut n = 0;
    for &b in bytes {
        if b == b'#' {
            n += 1;
        } else {
            break;
        }
    }
    if n == 0 || n > 6 {
        return None;
    }
    let after = &s[n..];
    if after.is_empty() {
        return None; // `#` alone is not a heading
    }
    if after.starts_with(' ') || after.starts_with('\t') {
        Some((n, n + 1))
    } else if after.starts_with('#') {
        None
    } else {
        None
    }
}

/// The `(start, end_incl)` lines touched by `sel` as `line_bounds` tuples.
fn touched_lines(text: &str, sel: &std::ops::Range<usize>) -> Vec<(usize, usize, usize)> {
    if text.is_empty() {
        // An empty buffer is still one (empty) line the caret can sit on.
        return vec![(0, 0, 0)];
    }
    let (a, b) = touched_line_bounds(text, sel);
    let mut out = Vec::new();
    let mut i = a;
    loop {
        match line_bounds(text, i) {
            Some(t) => {
                out.push(t);
                if i >= b {
                    break;
                }
                i += 1;
            }
            None => break,
        }
    }
    out
}

/// Assemble `out` lines + terminators and apply against the source block.
fn finish_lines(text: &str, lines: &[(usize, usize, usize)], out: &mut String, caret: usize) -> Out {
    let start = lines[0].0;
    let first_end = lines[0].1;
    let (_, _, end_incl) = *lines.last().unwrap();
    // Source block without a trailing newline (including an empty buffer): drop
    // our artificially added terminator so no blank line is invented.
    let had_terminator = end_incl > start && text[start..end_incl].ends_with('\n');
    if !had_terminator && out.ends_with('\n') {
        out.pop();
    }
    // Prefix edits shift the text under the caret: keep it on the same
    // character of the line (the line body itself is never rewritten).
    let new_first_len = out.split('\n').next().map_or(0, str::len);
    let caret = if caret >= start && caret <= first_end {
        let shifted = caret as isize + (new_first_len as isize - (first_end - start) as isize);
        shifted.clamp(start as isize, (start + new_first_len) as isize) as usize
    } else {
        caret
    };
    let sel = caret.min(start + out.len());
    apply(text, start..end_incl, out, sel..sel)
}

/// Toggle heading level `level` (1..=6) on every touched line (spec §6.19).
pub fn toggle_heading(text: &str, sel: &std::ops::Range<usize>, level: u8) -> Out {
    assert!((1..=6).contains(&level));
    let lines = touched_lines(text, sel);
    if lines.is_empty() {
        return Out::Noop("no text");
    }
    let mut out = String::new();
    for &(s, e, _) in &lines {
        let content = &text[s..e];
        if content.trim().is_empty() {
            // Caret on an empty line: drop in the marker so typing can start.
            if sel.is_empty() {
                out.push_str(&"#".repeat(level as usize));
                out.push(' ');
            }
            out.push('\n');
            continue;
        }
        match heading_prefix(content) {
            // Same level already → demote to a plain paragraph (repeat-to-remove).
            Some((n, skip)) if n == level as usize => {
                out.push_str(content[skip..].trim_start_matches([' ', '\t']));
            }
            Some((_, skip)) => {
                out.push_str(&"#".repeat(level as usize));
                out.push(' ');
                out.push_str(content[skip..].trim_start_matches([' ', '\t']));
            }
            None => {
                out.push_str(&"#".repeat(level as usize));
                out.push(' ');
                out.push_str(content);
            }
        }
        out.push('\n');
    }
    finish_lines(text, &lines, &mut out, sel.start)
}

fn strip_heading(s: &str) -> &str {
    match heading_prefix(s) {
        Some((_, skip)) => &s[skip..],
        None => s,
    }
}

fn quote_prefix(s: &str) -> Option<usize> {
    if s.starts_with("> ") {
        Some(2)
    } else if s == ">" {
        Some(1)
    } else {
        None
    }
}

/// Toggle blockquote on every touched line (spec §6.19).
pub fn toggle_blockquote(text: &str, sel: &std::ops::Range<usize>) -> Out {
    let lines = touched_lines(text, sel);
    if lines.is_empty() {
        return Out::Noop("no text");
    }
    let all_quoted = lines.iter().all(|&(s, e, _)| {
        let c = &text[s..e];
        quote_prefix(c).is_some() || c.trim().is_empty()
    });
    let mut out = String::new();
    for &(s, e, _) in &lines {
        let content = &text[s..e];
        if content.trim().is_empty() {
            if sel.is_empty() {
                out.push_str("> ");
            }
            out.push('\n');
        } else if all_quoted {
            match quote_prefix(content) {
                Some(skip) => out.push_str(&content[skip..]),
                None => out.push_str(content),
            }
            out.push('\n');
        } else if quote_prefix(content).is_some() {
            out.push_str(content);
            out.push('\n');
        } else {
            out.push_str("> ");
            out.push_str(content);
            out.push('\n');
        }
    }
    finish_lines(text, &lines, &mut out, sel.start)
}

/// Detect a list/quote marker prefix at the start of a line.
fn list_prefix(s: &str) -> Option<usize> {
    let t = s.trim_start_matches(' ');
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        Some(s.len() - t.len() + 2)
    } else {
        None
    }
}

/// Toggle bullet lists on touched lines (accepts `-`, `*`, `+`).
pub fn toggle_bullet_list(text: &str, sel: &std::ops::Range<usize>) -> Out {
    let lines = touched_lines(text, sel);
    if lines.is_empty() {
        return Out::Noop("no text");
    }
    let all_list = lines.iter().all(|&(s, e, _)| {
        let c = &text[s..e];
        list_prefix(c).is_some() || c.trim().is_empty()
    });
    let mut out = String::new();
    for &(s, e, _) in &lines {
        let content = &text[s..e];
        if content.trim().is_empty() {
            if sel.is_empty() {
                out.push_str("- ");
            }
            out.push('\n');
        } else if all_list {
            match list_prefix(content) {
                Some(skip) => out.push_str(&content[skip..]),
                None => out.push_str(content),
            }
            out.push('\n');
        } else if list_prefix(content).is_some() {
            out.push_str(content);
            out.push('\n');
        } else {
            out.push_str("- ");
            out.push_str(content);
            out.push('\n');
        }
    }
    finish_lines(text, &lines, &mut out, sel.start)
}

fn numbered_prefix(s: &str) -> Option<usize> {
    let t = s.trim_start_matches(' ');
    let digits: usize = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && t[digits..].starts_with(". ") {
        Some(s.len() - t.len() + digits + 2)
    } else {
        None
    }
}

/// Toggle `1. ` numbered lists (GFM renders the true sequence regardless of literals).
pub fn toggle_numbered_list(text: &str, sel: &std::ops::Range<usize>) -> Out {
    let lines = touched_lines(text, sel);
    if lines.is_empty() {
        return Out::Noop("no text");
    }
    let all_list = lines.iter().all(|&(s, e, _)| {
        let c = &text[s..e];
        numbered_prefix(c).is_some() || c.trim().is_empty()
    });
    let mut out = String::new();
    for &(s, e, _) in &lines {
        let content = &text[s..e];
        if content.trim().is_empty() {
            if sel.is_empty() {
                out.push_str("1. ");
            }
            out.push('\n');
        } else if all_list {
            match numbered_prefix(content) {
                Some(skip) => out.push_str(&content[skip..]),
                None => out.push_str(content),
            }
            out.push('\n');
        } else if numbered_prefix(content).is_some() {
            out.push_str(content);
            out.push('\n');
        } else {
            out.push_str("1. ");
            out.push_str(content);
            out.push('\n');
        }
    }
    finish_lines(text, &lines, &mut out, sel.start)
}

fn is_task_line(content: &str) -> bool {
    content.trim_start().starts_with("- [")
}

/// Toggle task-list markers `- [ ] ` / `- [x] ` on touched lines.
pub fn toggle_task_list(text: &str, sel: &std::ops::Range<usize>, checked: bool) -> Out {
    let lines = touched_lines(text, sel);
    if lines.is_empty() {
        return Out::Noop("no text");
    }
    let all_task = lines
        .iter()
        .all(|&(s, e, _)| { let c = &text[s..e]; is_task_line(c) || c.trim().is_empty() });
    let marker = if checked { "- [x] " } else { "- [ ] " };

    let mut out = String::new();
    for &(s, e, _) in &lines {
        let content = &text[s..e];
        if content.trim().is_empty() {
            if sel.is_empty() {
                out.push_str(marker);
            }
            out.push('\n');
            continue;
        }
        let lead = content.len() - content.trim_start_matches(' ').len();
        if all_task && is_task_line(content) {
            // Drop the checkbox, keep the `- ` bullet (spec: toggling a task
            // back drops the `[ ]`).
            let rest = &content[lead..];
            let after_box = rest.find(']').map(|i| i + 1).unwrap_or(2);
            let tail = rest[after_box..].trim_start_matches([' ', '\t']);
            out.push_str(&content[..lead]);
            out.push_str("- ");
            out.push_str(tail);
        } else if is_task_line(content) {
            out.push_str(content);
        } else if list_prefix(content).is_some() {
            // Promote an existing bullet to a task.
            let rest = &content[lead..];
            out.push_str(&content[..lead]);
            out.push_str(marker);
            out.push_str(&rest[2..]); // skip "- "
        } else {
            out.push_str(marker);
            out.push_str(content);
        }
        out.push('\n');
    }
    finish_lines(text, &lines, &mut out, sel.start)
}

struct ListLineInfo<'a> {
    indent: &'a str,
    prefix_len: usize,
    next_marker: String,
    is_empty: bool,
}

fn detect_list_line(line: &str) -> Option<ListLineInfo<'_>> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let indent = &line[..indent_len];
    let rest = &line[indent_len..];

    // 1. Task list item: e.g. "- [ ] ", "- [x] ", "* [ ] ", "+ [ ] "
    if (rest.starts_with("- [") || rest.starts_with("* [") || rest.starts_with("+ [")) && rest.len() >= 5 {
        let b = rest.as_bytes();
        if b[2] == b'[' && (b[3] == b' ' || b[3] == b'x' || b[3] == b'X') && b[4] == b']' {
            let bullet = &rest[..2];
            let marker_len = if rest.len() >= 6 && b[5] == b' ' { 6 } else { 5 };
            let content = &rest[marker_len..];
            return Some(ListLineInfo {
                indent,
                prefix_len: indent_len + marker_len,
                next_marker: format!("{}[ ] ", bullet),
                is_empty: content.trim().is_empty(),
            });
        }
    }

    // 2. Bullet list item: "- ", "* ", "+ "
    if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        let bullet = &rest[..2];
        let content = &rest[2..];
        return Some(ListLineInfo {
            indent,
            prefix_len: indent_len + 2,
            next_marker: bullet.to_string(),
            is_empty: content.trim().is_empty(),
        });
    }

    // 3. Numbered list item: "1. ", "1) ", etc.
    let digits_len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits_len > 0 {
        let after_digits = &rest[digits_len..];
        if after_digits.starts_with(". ") || after_digits.starts_with(") ") {
            let delim = &after_digits[..2];
            let num: u64 = rest[..digits_len].parse().unwrap_or(0);
            let marker_len = digits_len + 2;
            let content = &rest[marker_len..];
            return Some(ListLineInfo {
                indent,
                prefix_len: indent_len + marker_len,
                next_marker: format!("{}{}", num.saturating_add(1), delim),
                is_empty: content.trim().is_empty(),
            });
        } else if after_digits == "." || after_digits == ")" {
            let delim = if after_digits == "." { ". " } else { ") " };
            let num: u64 = rest[..digits_len].parse().unwrap_or(0);
            let marker_len = digits_len + 1;
            return Some(ListLineInfo {
                indent,
                prefix_len: indent_len + marker_len,
                next_marker: format!("{}{}", num.saturating_add(1), delim),
                is_empty: true,
            });
        }
    }

    None
}

/// Smart Enter behavior for bullet, numbered, and task lists.
///
/// If `caret` is on an empty list item (only prefix/spaces on the line), hitting
/// Enter removes the marker, exiting the list.
/// If `caret` is within or at the end of a non-empty list item's content, hitting
/// Enter creates the next list item (incrementing the number for numbered lists,
/// unchecked for task lists, preserving indentation and bullet style).
/// Returns `None` if the line is not a list item or caret is before the list marker.
pub fn smart_enter(text: &str, caret: usize) -> Option<Op> {
    let caret = caret.min(text.len());
    let line_start = text[..caret].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[caret..].find('\n').map(|i| caret + i).unwrap_or(text.len());
    let mut effective_end = line_end;
    if effective_end > line_start && text.as_bytes()[effective_end - 1] == b'\r' {
        effective_end -= 1;
    }
    let line = &text[line_start..effective_end];

    let info = detect_list_line(line)?;
    let col = caret - line_start;

    if info.is_empty {
        let mut new_text = String::with_capacity(text.len());
        new_text.push_str(&text[..line_start]);
        new_text.push_str(&text[effective_end..]);
        let new_caret = line_start;
        Some(Op { new_text, selection: new_caret..new_caret })
    } else if col >= info.prefix_len {
        let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let insertion = format!("{}{}{}", nl, info.indent, info.next_marker);
        let mut new_text = String::with_capacity(text.len() + insertion.len());
        new_text.push_str(&text[..caret]);
        new_text.push_str(&insertion);
        new_text.push_str(&text[caret..]);
        let new_caret = caret + insertion.len();
        Some(Op { new_text, selection: new_caret..new_caret })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Links & images (spec §6.22 / §6.21)
// ---------------------------------------------------------------------------

/// Insert `[display](url)` at the caret; select the `display` part for editing.
pub fn insert_link(text: &str, caret: usize, display: &str, url: &str) -> Op {
    let md = format!("[{}]({})", display, url);
    let sel_start = caret + 1; // inside the brackets
    let sel_end = sel_start + display.chars().count();
    let op = Op { new_text: replace_range(text, caret..caret, &md), selection: sel_start..sel_end };
    op
}

/// Insert `![alt](url)` at the caret; select the alt text.
pub fn insert_image(text: &str, caret: usize, alt: &str, url: &str) -> Op {
    let norm = url.replace('\\', "/");
    let md = if norm.contains(' ') {
        format!("![{}](<{}>)", alt, norm)
    } else {
        format!("![{}]({})", alt, norm)
    };
    let sel_start = caret + 2; // inside ![ ]
    let sel_end = sel_start + alt.chars().count();
    let op = Op { new_text: replace_range(text, caret..caret, &md), selection: sel_start..sel_end };
    op
}

// ---------------------------------------------------------------------------
// Tables (spec §6.20)
// ---------------------------------------------------------------------------

/// A parsed GFM table within the source text.
struct RawTable {
    /// Byte range covering header..=last body line (incl. newlines).
    start: usize,
    end: usize,
    header: Vec<String>,
    body: Vec<Vec<String>>,
    aligns: Vec<Align>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
    None,
}

fn is_delim_line(s: &str) -> bool {
    let t = s.trim().trim_start_matches('|').trim_end_matches('|');
    if t.is_empty() {
        return false;
    }
    t.split('|').all(|cell| {
        let c = cell.trim();
        !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
            && c.contains('-')
    })
}

fn split_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for ch in line.trim().chars() {
        if escaped {
            cur.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '|' {
            cells.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(ch);
        }
    }
    cells.push(cur.trim().to_string());
    // GFM rows conventionally start/end with `|`; drop those empty boundaries.
    if cells.len() > 1 && cells.first().map(|c| c.is_empty()).unwrap_or(false) {
        cells.remove(0);
    }
    if cells.len() > 1 && cells.last().map(|c| c.is_empty()).unwrap_or(false) {
        cells.pop();
    }
    cells
}

fn parse_tables(text: &str) -> Vec<RawTable> {
    let mut lines: Vec<(usize, usize, usize)> = Vec::new();
    let mut i = 0;
    while let Some(b) = line_bounds(text, i) {
        lines.push(b);
        i += 1;
    }
    let total = lines.len();
    if total < 2 {
        return Vec::new();
    }
    let mut tables = Vec::new();
    let mut idx = 0;
    while idx + 1 < total {
        let (hs, he, _) = lines[idx];
        let header_raw = &text[hs..he];
        if !header_raw.contains('|') {
            idx += 1;
            continue;
        }
        let (ds, de, _) = lines[idx + 1];
        if !is_delim_line(&text[ds..de]) {
            idx += 1;
            continue;
        }
        let mut body_idx = idx + 2;
        let mut body: Vec<Vec<String>> = Vec::new();
        while body_idx < total {
            let (bs, be, _) = lines[body_idx];
            let content = &text[bs..be];
            if !content.contains('|') || content.trim().is_empty() {
                break;
            }
            body.push(split_cells(content));
            body_idx += 1;
        }
        let last_body = if body.is_empty() { idx + 1 } else { body_idx - 1 };
        let (_, _, le) = lines[last_body];
        let header = split_cells(header_raw);
        let delim_cells = split_cells(&text[ds..de]);
        let aligns = delim_cells.iter().map(|c| {
            let t = c.trim();
            if t.starts_with(':') && t.ends_with(':') { Align::Center }
            else if t.ends_with(':') { Align::Right }
            else if t.starts_with(':') { Align::Left }
            else { Align::None }
        }).collect();
        tables.push(RawTable { start: hs, end: le, header, body, aligns });
        idx = body_idx;
    }
    tables
}

fn table_cols(t: &RawTable) -> usize {
    t.header.len().max(t.body.iter().map(|r| r.len()).max().unwrap_or(0)).max(t.aligns.len())
}

/// Find the table containing `byte`, if any, and the 0-based table row:
/// 0 = header, 1 = delimiter, 2.. = body rows.
fn table_at(text: &str, byte: usize) -> Option<(RawTable, usize)> {
    let tables = parse_tables(text);
    for t in tables {
        let is_inside = if byte >= t.start && byte < t.end {
            true
        } else if byte == t.end {
            byte <= text.len()
        } else {
            false
        };
        if is_inside {
            let caret_line = line_index(text, byte.min(t.end.saturating_sub(1)));
            let table_start_line = line_index(text, t.start);
            let total_table_lines = 2 + t.body.len();
            let row = caret_line.saturating_sub(table_start_line).min(total_table_lines.saturating_sub(1));
            return Some((t, row));
        }
    }
    None
}

/// Render the table block with padded cells.
fn render_table(header: &[String], body: &[Vec<String>], aligns: &[Align]) -> String {
    let cols = header.len().max(aligns.len()).max(body.iter().map(|r| r.len()).max().unwrap_or(0)).max(1);
    let mut widths = vec![3usize; cols];
    let mut calc = |cells: &[String]| {
        for (i, c) in cells.iter().enumerate() {
            if i < cols {
                widths[i] = widths[i].max(c.chars().count());
            }
        }
    };
    calc(header);
    for r in body {
        calc(r);
    }
    let mut s = String::new();
    let row = |cells: &[String], s: &mut String| {
        s.push('|');
        for i in 0..cols {
            let content = cells.get(i).map(|c| c.as_str()).unwrap_or("");
            s.push(' ');
            s.push_str(content);
            for _ in content.chars().count()..widths[i] {
                s.push(' ');
            }
            s.push_str(" |");
        }
        s.push('\n');
    };
    row(header, &mut s);
    // Delimiter row
    s.push('|');
    for i in 0..cols {
        let a = aligns.get(i).copied().unwrap_or(Align::None);
        s.push(' ');
        let w = widths[i];
        match a {
            Align::Left => {
                s.push(':');
                for _ in 0..w.saturating_sub(1) {
                    s.push('-');
                }
            }
            Align::Right => {
                for _ in 0..w.saturating_sub(1) {
                    s.push('-');
                }
                s.push(':');
            }
            Align::Center => {
                s.push(':');
                for _ in 0..w.saturating_sub(2).max(1) {
                    s.push('-');
                }
                s.push(':');
            }
            Align::None => {
                for _ in 0..w {
                    s.push('-');
                }
            }
        }
        s.push_str(" |");
    }
    s.push('\n');
    for r in body {
        row(r, &mut s);
    }
    s
}

/// Insert a new `rows × cols` table at the caret (spec A3).
pub fn insert_table(text: &str, caret: usize, rows: usize, cols: usize) -> Op {
    let rows = rows.max(1);
    let cols = cols.max(1);
    let header: Vec<String> = (1..=cols).map(|i| format!("Column {}", i)).collect();
    let body: Vec<Vec<String>> = (0..rows).map(|_| vec![String::new(); cols]).collect();
    let aligns = vec![Align::None; cols];
    let rendered = render_table(&header, &body, &aligns);
    let op = Op { new_text: replace_range(text, caret..caret, &rendered), selection: caret + rendered.len()..caret + rendered.len() };
    op
}

/// Row/column operations on the table containing `caret`.
pub fn table_op(text: &str, caret: usize, op: TableCmd) -> Out {
    let Some((t, row)) = table_at(text, caret) else {
        return Out::Noop("caret is not inside a table");
    };
    let cols = table_cols(&t);
    let mut header = t.header.clone();
    let mut body = t.body.clone();
    let mut aligns = t.aligns.clone();
    while aligns.len() < cols {
        aligns.push(Align::None);
    }
    let pad = |cells: &mut Vec<String>| {
        while cells.len() < cols {
            cells.push(String::new());
        }
    };
    pad(&mut header);
    for r in &mut body {
        pad(r);
    }
    let col = table_col_of(text, caret, &t).min(cols.saturating_sub(1));
    match op {
        TableCmd::InsertRowBelow => {
            let new_row = vec![String::new(); cols];
            if row <= 1 {
                body.insert(0, new_row);
            } else {
                let body_idx = row - 2;
                let at = (body_idx + 1).min(body.len());
                body.insert(at, new_row);
            }
        }
        TableCmd::InsertRowAbove => {
            if row == 0 {
                let old = std::mem::take(&mut header);
                body.insert(0, old);
                header = vec![String::new(); cols];
            } else if row == 1 {
                body.insert(0, vec![String::new(); cols]);
            } else {
                let body_idx = row - 2;
                let at = body_idx.min(body.len());
                body.insert(at, vec![String::new(); cols]);
            }
        }
        TableCmd::InsertColRight => {
            let at = (col + 1).min(header.len());
            header.insert(at, String::new());
            for r in &mut body {
                let r_at = at.min(r.len());
                r.insert(r_at, String::new());
            }
            let a_at = at.min(aligns.len());
            aligns.insert(a_at, Align::None);
        }
        TableCmd::InsertColLeft => {
            let at = col.min(header.len());
            header.insert(at, String::new());
            for r in &mut body {
                let r_at = at.min(r.len());
                r.insert(r_at, String::new());
            }
            let a_at = at.min(aligns.len());
            aligns.insert(a_at, Align::None);
        }
        TableCmd::DeleteRow => {
            if row == 0 {
                // Deleting header row: body first row becomes the header.
                if !body.is_empty() {
                    header = body.remove(0);
                } else {
                    header = vec![String::new(); cols];
                }
            } else if row == 1 {
                if !body.is_empty() {
                    if body.len() <= 1 {
                        body = vec![vec![String::new(); cols]];
                    } else {
                        body.remove(0);
                    }
                }
            } else {
                let body_idx = row - 2;
                if body.len() <= 1 {
                    // keep at least one empty body row (spec B5)
                    body = vec![vec![String::new(); cols]];
                } else {
                    let at = body_idx.min(body.len() - 1);
                    body.remove(at);
                }
            }
        }
        TableCmd::DeleteCol => {
            if cols <= 1 {
                // Deleting the only column removes the whole table (spec B6).
                return apply(text, t.start..t.end, "", t.start..t.start);
            }
            if col < header.len() {
                header.remove(col);
            }
            for r in &mut body {
                if col < r.len() {
                    r.remove(col);
                }
            }
            if aligns.len() > col {
                aligns.remove(col);
            }
        }
        TableCmd::AlignCol(a) => {
            while aligns.len() <= col {
                aligns.push(Align::None);
            }
            aligns[col] = a;
        }
    }
    let rendered = render_table(&header, &body, &aligns);
    let had_newline = text[..t.end.min(text.len())].ends_with('\n');
    let mut block = rendered;
    if !had_newline && block.ends_with('\n') {
        block.pop();
    }
    let sel = caret.min(t.start + block.len());
    apply(text, t.start..t.end, &block, sel..sel)
}

fn table_col_of(text: &str, byte: usize, _t: &RawTable) -> usize {
    let li = line_index(text, byte);
    let (s, e, _) = line_bounds(text, li).unwrap_or((0, 0, 0));
    let raw = &text[s..e];
    // Count pipes before the caret inside this line.
    let col_offset = byte.min(e).saturating_sub(s).min(raw.len());
    let before = &raw[..col_offset];
    let pipes = before.bytes().filter(|&b| b == b'|').count();
    if pipes == 0 {
        0
    } else {
        pipes - 1
    }
}

#[derive(Clone, Copy)]
pub enum TableCmd {
    InsertRowBelow,
    InsertRowAbove,
    InsertColRight,
    InsertColLeft,
    DeleteRow,
    DeleteCol,
    AlignCol(Align),
}

/// Find a task item `[ ]`/`[x]` on the caret line and flip it (spec §6.7 P1).
/// Find a task item `[ ]`/`[x]` on the caret line and flip it (spec §6.7 P1).
pub fn toggle_task_item(text: &str, caret: usize) -> Option<Op> {
    let li = line_index(text, caret);
    let (s, e, _) = line_bounds(text, li)?;
    let line = &text[s..e];
    let idx = line.find("[ ]")?;
    let at = s + idx;
    let new_text = replace_range(text, at..at + 3, "[x]");
    let selection = (at + 1)..(at + 2);
    Some(Op { new_text, selection })
}

/// Remove markdown source formatting so a heading/quote/list block can be
/// copied as clean text (used by preview "copy source" flows). Kept tiny.
pub fn plain_line(line: &str) -> String {
    let mut s = line.to_string();
    if let Some((_, _)) = heading_prefix(line) {
        s = strip_heading(line).to_string();
    }
    if let Some(skip) = quote_prefix(&s) {
        s = s[skip..].to_string();
    }
    s
}

/// Convenience: total lines available for caret navigation sanity checks.
pub fn _lines_available(text: &str) -> usize {
    line_count(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_roundtrip() {
        let text = "say hello now";
        // Select "hello"
        let sel = 4..9;
        let Out::Applied(wrapped) = toggle_wrap(text, &sel, "**", "**") else { panic!("wrap failed") };
        assert_eq!(wrapped.new_text, "say **hello** now");
        // Second press on the whole token unwraps back.
        let sel2 = wrapped.selection.clone();
        let Out::Applied(unwrapped) = toggle_wrap(&wrapped.new_text, &sel2, "**", "**") else { panic!("unwrap failed") };
        assert_eq!(unwrapped.new_text, "say hello now");
    }

    #[test]
    fn bold_never_doubles_on_full_token() {
        let text = "**bold** here";
        let sel = 0..8;
        let Out::Applied(op) = toggle_wrap(text, &sel, "**", "**") else { panic!() };
        assert_eq!(op.new_text, "bold here");
    }

    #[test]
    fn caret_inside_token_unwraps() {
        let text = "a **bold** c";
        // caret at index inside "bold"
        let Out::Applied(op) = toggle_wrap(text, &(5..5), "**", "**") else { panic!() };
        assert_eq!(op.new_text, "a bold c");
    }

    #[test]
    fn empty_pair_insert() {
        let text = "ab";
        let Out::Applied(op) = toggle_wrap(text, &(1..1), "**", "**") else { panic!() };
        assert_eq!(op.new_text, "a****b");
        // caret sits between the two empty delimiters: a|**|**b → index 3
        assert_eq!(op.selection, 3..3);
    }

    #[test]
    fn heading_toggle_and_demote() {
        let text = "hello\nworld";
        let Out::Applied(h1) = toggle_heading(text, &(0..5), 1) else { panic!() };
        assert_eq!(h1.new_text, "# hello\nworld");
        let Out::Applied(h2) = toggle_heading(&h1.new_text, &(0..7), 2) else { panic!() };
        assert_eq!(h2.new_text, "## hello\nworld");
        let Out::Applied(plain) = toggle_heading(&h2.new_text, &(0..9), 2) else { panic!() };
        assert_eq!(plain.new_text, "hello\nworld");
    }

    #[test]
    fn blockquote_toggle() {
        let text = "line one\nline two";
        let Out::Applied(q) = toggle_blockquote(text, &(0..text.len())) else { panic!() };
        assert_eq!(q.new_text, "> line one\n> line two");
        let Out::Applied(un) = toggle_blockquote(&q.new_text, &(0..q.new_text.len())) else { panic!() };
        assert_eq!(un.new_text, "line one\nline two");
    }

    #[test]
    fn bullet_toggle() {
        let text = "a\nb";
        let Out::Applied(l) = toggle_bullet_list(text, &(0..text.len())) else { panic!() };
        assert_eq!(l.new_text, "- a\n- b");
    }

    #[test]
    fn insert_link_basic() {
        let text = "abc";
        let op = insert_link(text, 3, "click", "https://x.dev");
        assert_eq!(op.new_text, "abc[click](https://x.dev)");
        // selection = the display text inside brackets
        assert_eq!(&op.new_text[op.selection.clone()], "click");
    }

    #[test]
    fn table_insert_and_ops() {
        let text = "";
        let op = insert_table(text, 0, 2, 3);
        assert!(op.new_text.contains("| Column 1 | Column 2 | Column 3 |"));
        assert_eq!(op.new_text.lines().count(), 4, "header + delim + 2 body rows");
        // Put the caret on the first body row, then add a row below it.
        let t2 = &op.new_text;
        let body_start = t2.find("|  |  |").or_else(|| t2.rfind('\n').map(|i| i + 1)).unwrap_or(t2.len() - 1);
        let Out::Applied(with_row) = table_op(t2, body_start, TableCmd::InsertRowBelow) else { panic!("not in table") };
        assert_eq!(with_row.new_text.lines().count(), 5, "one extra row added");
        let Out::Applied(with_col) = table_op(&with_row.new_text, 0, TableCmd::InsertColLeft) else { panic!() };
        let pipes = with_col.new_text.lines().next().unwrap().matches('|').count();
        assert_eq!(pipes, 5, "4 columns render as 5 pipes on a row");
    }

    #[test]
    fn delete_only_col_removes_table() {
        let t = "| A |\n| --- |\n| 1 |\n";
        let Out::Applied(op) = table_op(t, 0, TableCmd::DeleteCol) else { panic!() };
        assert!(op.new_text.trim().is_empty());
    }

    #[test]
    fn task_item_flip() {
        let t = "- [ ] do it";
        let r = toggle_task_item(t, 2);
        assert!(
            r.is_some(),
            "no task found: line={:?} find={:?}",
            line_bounds(t, 2).map(|(s, e, _)| &t[s..e]),
            t.find("[ ]")
        );
        let op = r.unwrap();
        assert_eq!(op.new_text, "- [x] do it");
    }

    // --- regression: block commands at the buffer edges ---------------------
    // (H1/H2 used to panic: `touched_line_bounds` probed one byte past the
    // caret and could slice inside a multibyte character.)

    #[test]
    fn heading_with_caret_at_eof() {
        let Out::Applied(op) = toggle_heading("hello", &(5..5), 1) else { panic!("no-op at EOF") };
        assert_eq!(op.new_text, "# hello");
        assert_eq!(op.selection, 7..7, "caret must stay at the end of the line");
    }

    #[test]
    fn heading_on_empty_document_inserts_prefix() {
        let Out::Applied(op) = toggle_heading("", &(0..0), 2) else { panic!("no-op on empty doc") };
        assert_eq!(op.new_text, "## ");
        assert_eq!(op.selection, 3..3);
        // repeat press removes the empty prefix again
        let Out::Applied(off) = toggle_heading(&op.new_text, &op.selection, 2) else { panic!("no-op") };
        assert_eq!(off.new_text, "");
    }

    #[test]
    fn heading_on_empty_middle_line_prefixes_that_line_only() {
        let text = "a\n\nb";
        let Out::Applied(op) = toggle_heading(text, &(2..2), 1) else { panic!("no-op") };
        assert_eq!(op.new_text, "a\n# \nb");
        assert_eq!(op.selection.start, 4, "caret lands after the inserted prefix");
    }

    #[test]
    fn heading_keeps_relative_caret_on_a_plain_line() {
        let Out::Applied(op) = toggle_heading("hello world", &(6..6), 1) else { panic!("no-op") };
        assert_eq!(op.new_text, "# hello world");
        assert_eq!(&op.new_text[..op.selection.start], "# hello ");
    }

    #[test]
    fn block_toggles_survive_buffer_edges() {
        // Caret at the very end of the buffer (no trailing newline) — every
        // block command must still apply instead of panicking.
        let t = "héllo 😀";
        let eof = t.len();
        for (name, out) in [
            ("heading", toggle_heading(t, &(eof..eof), 1)),
            ("quote", toggle_blockquote(t, &(eof..eof))),
            ("bullet", toggle_bullet_list(t, &(eof..eof))),
            ("numbered", toggle_numbered_list(t, &(eof..eof))),
            ("task", toggle_task_list(t, &(eof..eof), false)),
        ] {
            assert!(matches!(out, Out::Applied(_)), "{name} refused at EOF");
        }
        // Caret on the first byte of a multibyte char (byte 1 is mid-glyph).
        let Out::Applied(_) = toggle_bullet_list("é", &(0..0)) else { panic!("bullet refused on multibyte line") };
    }

    #[test]
    fn smart_enter_bullet_continuation_and_exit() {
        // 1. Enter on a bullet line with text creates the next bullet.
        let text = "- first item";
        let op = smart_enter(text, text.len()).expect("expected smart enter");
        assert_eq!(op.new_text, "- first item\n- ");
        assert_eq!(op.selection, 15..15);

        // 2. Enter on an empty bullet line takes out the empty point.
        let op2 = smart_enter(&op.new_text, op.selection.start).expect("expected smart enter exit");
        assert_eq!(op2.new_text, "- first item\n");
        assert_eq!(op2.selection, 13..13);

        // 3. Asterisk bullet style is preserved.
        let star = "* star item";
        let op_star = smart_enter(star, star.len()).expect("expected star enter");
        assert_eq!(op_star.new_text, "* star item\n* ");

        // 4. Plus bullet style is preserved.
        let plus = "+ plus item";
        let op_plus = smart_enter(plus, plus.len()).expect("expected plus enter");
        assert_eq!(op_plus.new_text, "+ plus item\n+ ");

        // 5. Indented bullet preserves indentation and bullet style.
        let indented = "  - sub item";
        let op_ind = smart_enter(indented, indented.len()).expect("expected indented enter");
        assert_eq!(op_ind.new_text, "  - sub item\n  - ");

        // 6. Enter on empty indented bullet takes out the empty sub-point.
        let op_ind_exit = smart_enter(&op_ind.new_text, op_ind.selection.start).expect("expected indented exit");
        assert_eq!(op_ind_exit.new_text, "  - sub item\n");
    }

    #[test]
    fn smart_enter_numbered_continuation_and_exit() {
        // 1. Enter on numbered line increments the number.
        let text = "1. first";
        let op = smart_enter(text, text.len()).expect("expected numbered enter");
        assert_eq!(op.new_text, "1. first\n2. ");
        assert_eq!(op.selection, 12..12);

        // 2. Enter on empty numbered line takes out the empty number point.
        let op2 = smart_enter(&op.new_text, op.selection.start).expect("expected numbered exit");
        assert_eq!(op2.new_text, "1. first\n");
        assert_eq!(op2.selection, 9..9);

        // 3. Multi-digit numbers and parenthesis delimiters work.
        let multi = "9. ninth";
        let op_multi = smart_enter(multi, multi.len()).expect("expected 10.");
        assert_eq!(op_multi.new_text, "9. ninth\n10. ");

        let paren = "1) first paren";
        let op_paren = smart_enter(paren, paren.len()).expect("expected 2)");
        assert_eq!(op_paren.new_text, "1) first paren\n2) ");

        // 4. Indented numbered list.
        let ind_num = "    3. four";
        let op_ind_num = smart_enter(ind_num, ind_num.len()).expect("expected 4.");
        assert_eq!(op_ind_num.new_text, "    3. four\n    4. ");
    }

    #[test]
    fn smart_enter_task_continuation_and_exit() {
        let task = "- [x] completed task";
        let op = smart_enter(task, task.len()).expect("expected task enter");
        assert_eq!(op.new_text, "- [x] completed task\n- [ ] ");

        let op2 = smart_enter(&op.new_text, op.selection.start).expect("expected task exit");
        assert_eq!(op2.new_text, "- [x] completed task\n");
    }

    #[test]
    fn smart_enter_mid_item_and_plain_text() {
        // Splitting in the middle of a bullet item.
        let text = "- hello world";
        let op = smart_enter(text, 7).expect("split item");
        assert_eq!(op.new_text, "- hello\n-  world");

        // Plain text returns None.
        assert!(smart_enter("plain text", 5).is_none());

        // Horizontal rule returns None.
        assert!(smart_enter("---", 3).is_none());

        // Caret before prefix on a non-empty item returns None.
        assert!(smart_enter("- hello", 0).is_none());
    }

    #[test]
    fn test_table_operations_robustness() {
        let text = "# Title\n\nSome text\n\n| Column 1 | Column 2 |\n| --- | --- |\n| a | b |\n";
        let caret = text.find("| a | b |").unwrap();

        // Insert row above and below inside body row
        let Out::Applied(above) = table_op(text, caret, TableCmd::InsertRowAbove) else { panic!() };
        assert_eq!(above.new_text.lines().count(), 8, "added 1 row above");
        let Out::Applied(below) = table_op(text, caret, TableCmd::InsertRowBelow) else { panic!() };
        assert_eq!(below.new_text.lines().count(), 8, "added 1 row below");

        // Insert column left and right inside body row
        let Out::Applied(col_l) = table_op(text, caret, TableCmd::InsertColLeft) else { panic!() };
        assert_eq!(table_cols(&parse_tables(&col_l.new_text)[0]), 3);
        let Out::Applied(col_r) = table_op(text, caret, TableCmd::InsertColRight) else { panic!() };
        assert_eq!(table_cols(&parse_tables(&col_r.new_text)[0]), 3);

        // Table operations when caret is at the end of the table
        let at_end = text.trim_end().len();
        let Out::Applied(row_end) = table_op(text, at_end, TableCmd::InsertRowBelow) else { panic!() };
        assert_eq!(row_end.new_text.lines().count(), 8);

        // Operations on header and delimiter
        let header_pos = text.find("| Column 1").unwrap();
        let Out::Applied(h_below) = table_op(text, header_pos, TableCmd::InsertRowBelow) else { panic!() };
        assert_eq!(h_below.new_text.lines().count(), 8);
        let delim_pos = text.find("| ---").unwrap();
        let Out::Applied(d_below) = table_op(text, delim_pos, TableCmd::InsertRowBelow) else { panic!() };
        assert_eq!(d_below.new_text.lines().count(), 8);

        // Delete row
        let Out::Applied(del_r) = table_op(text, caret, TableCmd::DeleteRow) else { panic!() };
        assert_eq!(del_r.new_text.lines().count(), 7, "deleted body row but kept minimum 1 empty body row");
    }
}
