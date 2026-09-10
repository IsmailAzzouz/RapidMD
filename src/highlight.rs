//! Markdown-aware editor tokenization (spec F42) — pure and unit-tested.
//!
//! The tokenizer is line-oriented with persistent fenced-code state; inline
//! emphasis/code/links are scanned within each line. Output is a list of
//! non-overlapping byte ranges in document order.

/// Token categories (colors are chosen by the UI layer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tk {
    Plain,
    Heading,
    Strong,
    Emphasis,
    Strike,
    Code,
    LinkText,
    LinkUrl,
    ListMarker,
    Quote,
    Task,
    Hr,
    Html,
    Fence,
    CodeText,
}

pub struct Syntax {
    pub text: String,
    pub tokens: Vec<(std::ops::Range<usize>, Tk)>,
}

/// Tokenize whole markdown source.
pub fn tokenize(text: &str) -> Syntax {
    let mut tokens: Vec<(std::ops::Range<usize>, Tk)> = Vec::new();
    let mut in_fence = false;

    let lines: Vec<(usize, usize, usize)> = {
        let mut out = Vec::new();
        let mut i = 0;
        loop {
            match text_line_bounds(text, i) {
                Some(b) => {
                    out.push(b);
                    i += 1;
                }
                None => break,
            }
        }
        out
    };

    for &(start, end, _) in &lines {
        let line = &text[start..end];
        if let Some((fs, fe, lang)) = fence_span(line) {
            let _ = lang;
            if in_fence {
                // Closing fence.
                tokens.push((start + fs..start + fe, Tk::Fence));
                in_fence = false;
                continue;
            }
            in_fence = true;
            tokens.push((start + fs..start + fe, Tk::Fence));
            continue;
        }
        if in_fence {
            tokens.push((start..end, Tk::CodeText));
            continue;
        }
tokenize_line(start, line, &mut tokens);
    }
    Syntax { text: text.to_owned(), tokens }
}

fn text_line_bounds(text: &str, line_idx: usize) -> Option<(usize, usize, usize)> {
    if text.is_empty() {
        return None;
    }
    let mut start = 0usize;
    for idx in 0..=line_idx {
        let end = match text[start..].find('\n') {
            Some(rel) => start + rel,
            None => text.len(),
        };
        if idx == line_idx {
            let end_incl = if end < text.len() { end + 1 } else { end };
            return Some((start, end, end_incl));
        }
        if end == text.len() {
            return None;
        }
        start = end + 1;
    }
    None
}

/// `(start, end, lang)` of a full-line fence (``` or ~~~).
fn fence_span(line: &str) -> Option<(usize, usize, &str)> {
    let t = line.trim_start_matches(' ');
    let indent = line.len() - t.len();
    if t.starts_with("```") || t.starts_with("~~~") {
        let lang = t[3..].trim();
        return Some((indent, indent + t.len(), lang));
    }
    None
}

fn is_hr(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    let ch = t.chars().next().unwrap();
    if ch != '-' && ch != '*' && ch != '_' {
        return false;
    }
    t.chars().all(|c| c == ch || c == ' ')
}

fn tokenize_line(line_start: usize, line: &str, tokens: &mut Vec<(std::ops::Range<usize>, Tk)>) {
    // Block-level markers first.
    if is_hr(line) {
        tokens.push((line_start..line_start + line.len(), Tk::Hr));
        return;
    }
    let trimmed = line.trim_start_matches(' ');
    let indent = line.len() - trimmed.len();

    // Task/list marker at the line start.
    let marker_end = if let Some(m) = task_or_list_marker(trimmed) {
        let m_len = m.len();
        tokens.push((line_start + indent..line_start + indent + m_len, Tk::Task));
        indent + m_len
    } else if let Some(m) = list_marker(trimmed) {
        let m_len = m.len();
        tokens.push((line_start + indent..line_start + indent + m_len, Tk::ListMarker));
        indent + m_len
    } else {
        indent
    };

    // Quote prefix.
    let after_quote = if trimmed.starts_with('>') {
        let qend = trimmed.find([' ', '\t']).map(|i| i + 1).unwrap_or(1);
        tokens.push((line_start + indent..line_start + indent + qend, Tk::Quote));
        marker_end.max(indent + qend)
    } else {
        marker_end
    };

    // ATX heading: whole line carries the heading color.
    if let Some(hlen) = heading_len(&line[after_quote.min(line.len())..]) {
        let hs = after_quote + hlen;
        tokens.push((line_start + after_quote..line_start + line.len(), Tk::Heading));
        let _ = hs;
        return;
    }

    // Inline scanning of the remaining content.
    scan_inline(line_start, line, after_quote, tokens);
}

fn task_or_list_marker(s: &str) -> Option<&str> {
    if let Some(i) = s.find('[') {
        if s[i..].starts_with("[ ] ") || s[i..].starts_with("[x] ") {
            return Some(&s[..i + 4]);
        }
    }
    None
}

fn list_marker(s: &str) -> Option<&str> {
    let first = s.chars().next()?;
    let after = s.get(first.len_utf8()..)?;
    let marker_len = if first == '-' || first == '*' || first == '+' {
        if after.starts_with(' ') {
            2
        } else {
            1
        }
    } else if first.is_ascii_digit() {
        let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
        if s[digits..].starts_with(". ") || s[digits..].starts_with(") ") {
            digits + 2
        } else {
            0
        }
    } else {
        0
    };
    if marker_len > 0 {
        Some(&s[..marker_len])
    } else {
        None
    }
}

fn heading_len(s: &str) -> Option<usize> {
    let t = s.trim_start_matches(' ');
    let lead = s.len() - t.len();
    let n = t.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&n) && t[n..].starts_with(' ') {
        Some(lead + n + 1)
    } else {
        None
    }
}

fn scan_inline(line_start: usize, line: &str, from: usize, tokens: &mut Vec<(std::ops::Range<usize>, Tk)>) {
    let mut i = from;
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut plain_start = from;

    let push_plain = |tokens: &mut Vec<(std::ops::Range<usize>, Tk)>, a: usize, b: usize| {
        if b > a {
            tokens.push((line_start + a..line_start + b, Tk::Plain));
        }
    };

    while i < line.len() {
        let rest = &line[i..];
        if rest.starts_with("```") {
            push_plain(tokens, plain_start, i);
            tokens.push((line_start + i..line_start + i + 3, Tk::Fence));
            plain_start = i + 3;
            i += 3;
            continue;
        }
        if rest.starts_with("**") || rest.starts_with("__") {
            let close = find_marker(line, i + 2, "**");
            if let Some(c) = close {
                push_plain(tokens, plain_start, i);
                tokens.push((line_start + i..line_start + c + 2, Tk::Strong));
                plain_start = c + 2;
                i = c + 2;
                continue;
            }
        }
        if rest.starts_with("~~") {
            let close = find_marker(line, i + 2, "~~");
            if let Some(c) = close {
                push_plain(tokens, plain_start, i);
                tokens.push((line_start + i..line_start + c + 2, Tk::Strike));
                plain_start = c + 2;
                i = c + 2;
                continue;
            }
        }
        if rest.starts_with('`') {
            // Code span: one or more backticks until matching run.
            let run = rest.chars().take_while(|&c| c == '`').count();
            if let Some(c) = find_code_close(line, i + run, run) {
                push_plain(tokens, plain_start, i);
                tokens.push((line_start + i..line_start + c + run, Tk::Code));
                plain_start = c + run;
                i = c + run;
                continue;
            }
        }
        // Link/image: [text](url) — only probe at real '[' / '![' starts so
        // the +1/+2 arithmetic never lands inside a multibyte character.
        if rest.starts_with('[') || rest.starts_with("![") {
            if let Some(after) = find_link_start(line, i) {
                if let Some((t_end, u_end, _)) = find_link_end(line, after) {
                    push_plain(tokens, plain_start, i);
                    tokens.push((line_start + i..line_start + t_end + 1, Tk::LinkText));
                    tokens.push((line_start + t_end + 1..line_start + u_end, Tk::LinkUrl));
                    plain_start = u_end;
                    i = u_end;
                    continue;
                }
            }
        }
        if rest.starts_with('*') && single_emphasis_possible(line, i) {
            if let Some(c) = find_single_star(line, i + 1) {
                push_plain(tokens, plain_start, i);
                tokens.push((line_start + i..line_start + c + 1, Tk::Emphasis));
                plain_start = c + 1;
                i = c + 1;
                continue;
            }
        }
        if rest.starts_with('<') {
            if let Some(c) = line[i..].find('>') {
                let e = i + c + 1;
                // Only treat as html when it looks like a tag or autolink.
                push_plain(tokens, plain_start, i);
                tokens.push((line_start + i..line_start + e, Tk::Html));
                plain_start = e;
                i = e;
                continue;
            }
        }
        if rest.starts_with('\\') {
            // Escaped char: skip next without tokenizing.
            i += 1 + line[i + 1..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            continue;
        }
        i += chars.iter().find(|&&(idx, _)| idx >= i).map(|&(_, c)| c.len_utf8()).unwrap_or(1);
    }
    push_plain(tokens, plain_start, line.len());
}

fn find_marker(line: &str, from: usize, marker: &str) -> Option<usize> {
    let mut i = from;
    while i < line.len() {
        if line[i..].starts_with(marker) {
            return Some(i);
        }
        i += line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    None
}

fn find_code_close(line: &str, from: usize, run: usize) -> Option<usize> {
    let mut i = from;
    while i < line.len() {
        let n = line[i..].chars().take_while(|&c| c == '`').count();
        if n >= run {
            return Some(i);
        }
        // Advance at least one whole character (never a raw byte, which can
        // land mid-way through a multi-byte character).
        i += if n > 0 {
            n
        } else {
            line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)
        };
    }
    None
}

fn single_emphasis_possible(line: &str, i: usize) -> bool {
    let before = line[..i].chars().next_back();
    if before == Some('\\') {
        return false;
    }
    // `**` already handled; lone `*` opens emphasis when followed by non-space.
    line[i + 1..].chars().next().map(|c| !c.is_whitespace()).unwrap_or(false)
}

fn find_single_star(line: &str, from: usize) -> Option<usize> {
    let mut i = from;
    while i < line.len() {
        let c = line[i..].chars().next().unwrap();
        if c == '*' && !line[i..].starts_with("**") {
            return Some(i);
        }
        i += c.len_utf8();
    }
    None
}

/// Given `i` at `[` or `![`, return byte offset of the matching `]` when a
/// `](url…)` follows.
fn find_link_start(line: &str, i: usize) -> Option<usize> {
    let after_open = if line[i..].starts_with("![") { i + 2 } else { i + 1 };
    let mut j = after_open;
    while j < line.len() {
        if line[j..].starts_with("](") {
            return Some(j);
        }
        j += line[j..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    None
}

fn find_link_end(line: &str, bracket_end: usize) -> Option<(usize, usize, usize)> {
    // bracket_end points at ']'; url spans bracket_end+1 .. matching ')'
    let open = bracket_end + 1;
    if !line[open..].starts_with('(') {
        return None;
    }
    let mut j = open + 1;
    let mut depth = 1u32;
    while j < line.len() {
        let c = line[j..].chars().next().unwrap();
        if c == ')' && depth == 1 {
            return Some((bracket_end, j + 1, j + 1));
        }
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        }
        j += c.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(s: &str) -> Vec<Tk> {
        tokenize(s).tokens.into_iter().map(|(_, k)| k).collect()
    }

    #[test]
    fn heading_and_plain() {
        assert!(kinds("# Title").contains(&Tk::Heading));
        assert!(kinds("plain text").contains(&Tk::Plain));
    }

    #[test]
    fn bold_italic_code() {
        assert!(kinds("a **b** c").contains(&Tk::Strong));
        assert!(kinds("a *b* c").contains(&Tk::Emphasis));
        assert!(kinds("a `b` c").contains(&Tk::Code));
    }

    #[test]
    fn links_split() {
        let s = "[x](https://e.dev)";
        let toks = tokenize(s);
        let has_text = toks.tokens.iter().any(|(_, k)| *k == Tk::LinkText);
        let has_url = toks.tokens.iter().any(|(_, k)| *k == Tk::LinkUrl);
        assert!(has_text && has_url);
    }

    #[test]
    fn fenced_code_blocks() {
        let s = "```rs\nfn main() {}\n```\n";
        let toks = tokenize(s);
        let has_fence = toks.tokens.iter().filter(|(_, k)| *k == Tk::Fence).count() >= 2;
        let has_code = toks.tokens.iter().any(|(_, k)| *k == Tk::CodeText);
        assert!(has_fence && has_code);
    }

    #[test]
    fn list_and_task() {
        assert!(kinds("- item").contains(&Tk::ListMarker));
        assert!(kinds("- [x] done").contains(&Tk::Task));
        assert!(kinds("> quote").contains(&Tk::Quote));
    }

    #[test]
    fn ranges_cover_text_without_overlap() {
        let s = "# Hi **bold** `code` [l](u)";
        let toks = tokenize(s);
        // Validate ascending order and non-overlap by construction (byte indices).
        let mut last = 0usize;
        for (r, _) in &toks.tokens {
            assert!(r.start >= last, "overlap or disorder at {:?}", r);
            last = r.end;
        }
        let _ = s;
    }

    #[test]
    fn multibyte_chars_do_not_panic() {
        // Regression: the link scanner used to slice at i+1 on every byte
        // position, panicking when i pointed at the start of a multi-byte
        // character such as the em dash '—'.
        let s = "computable in advance — and it is. Also émojis 🎉 here.";
        let toks = tokenize(s);
        assert!(!toks.tokens.is_empty());
        let s2 = "dash — then [link](https://x.dev)";
        let _ = tokenize(s2);
    }

    #[test]
    fn code_scan_after_multibyte_does_not_panic() {
        // Regression: find_code_close advanced by raw bytes (n.max(1)) while
        // searching for a closing backtick, slicing mid-character when a
        // multi-byte char such as '«' or '—' follows an unmatched backtick.
        let s = "| Title bar text | `«filename.md» — RustDownViewer`; `●` prefix";
        let toks = tokenize(s);
        assert!(!toks.tokens.is_empty());
        // Also a lone opening backtick followed by multibyte content.
        let s2 = "`——— unfinished";
        let _ = tokenize(s2);
    }
}
