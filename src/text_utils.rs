//! Pure text helpers: byte/char offset conversion and line arithmetic.
//!
//! egui cursors use *character* offsets while Rust `String` slicing needs
//! *byte* offsets; every cross-boundary helper lives here and is unit tested.

/// Byte offset → character offset. `byte` must lie on a UTF-8 boundary.
pub fn byte_to_char(text: &str, byte: usize) -> usize {
    debug_assert!(byte <= text.len());
    text[..byte].chars().count()
}

/// Character offset → byte offset (clamped to `text.len()`).
pub fn char_to_byte(text: &str, ch: usize) -> usize {
    text.char_indices()
        .nth(ch)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Byte offset of the first byte of the line containing `byte`.
pub fn line_start(text: &str, byte: usize) -> usize {
    text[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// Byte offset one past the last character of the line containing `byte`
/// (excludes the line terminator).
pub fn line_end(text: &str, byte: usize) -> usize {
    match text[byte..].find('\n') {
        Some(rel) => byte + rel,
        None => text.len(),
    }
}

/// `(start, end_excl_terminator, end_incl_terminator)` for line `line_idx` (0-based).
/// `(start, end_excl_terminator, end_incl_terminator)` for line `line_idx` (0-based).
/// Returns `None` when `text` is empty or the index is out of range.
pub fn line_bounds(text: &str, line_idx: usize) -> Option<(usize, usize, usize)> {
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

/// Total number of logical lines (`text` may or may not end with newline).
pub fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.bytes().filter(|&b| b == b'\n').count() + 1
    }
}

/// Column (0-based, characters) of `byte` within its line.
pub fn line_column(text: &str, byte: usize) -> usize {
    let ls = line_start(text, byte);
    text[ls..byte].chars().count()
}
/// Index (0-based) of the line containing `byte`.
pub fn line_index(text: &str, byte: usize) -> usize {
    text[..byte].bytes().filter(|&b| b == b'\n').count()
}

/// Word-count helper (whitespace separated tokens).

/// The `(start, end)` byte range of the word or run of non-whitespace
/// characters containing `byte`. Returns `None` when the caret sits on whitespace.
pub fn word_range_at(text: &str, byte: usize) -> Option<(usize, usize)> {
    let is_word = |c: char| !c.is_whitespace();
    if byte >= text.len() {
        // Trailing whitespace: no word.
        return text
            .char_indices()
            .rev()
            .find(|&(_, c)| is_word(c))
            .map(|(i, c)| (i, i + c.len_utf8()));
    }
    if !is_word(text[byte..].chars().next().unwrap()) {
        return None;
    }
    let start = text[..byte]
        .char_indices()
        .rev()
        .find(|&(_, c)| !is_word(c))
        .map(|(i, _)| i + text[i..].chars().next().unwrap().len_utf8())
        .unwrap_or(0);
    let end = text[byte..]
        .char_indices()
        .find(|&(_, c)| !is_word(c))
        .map(|(i, _)| byte + i)
        .unwrap_or(text.len());
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_char_roundtrip() {
        let s = "héllo wörld";
        for (ch_i, _ch) in s.chars().enumerate() {
            let b = char_to_byte(s, ch_i);
            assert_eq!(byte_to_char(s, b), ch_i);
        }
        assert_eq!(byte_to_char(s, 0), 0);
        assert_eq!(char_to_byte(s, s.chars().count()), s.len());
    }

    #[test]
    fn lines() {
        let s = "aa\nbb\ncc";
        assert_eq!(line_count(s), 3);
        assert_eq!(line_bounds(s, 0), Some((0, 2, 3)));
        assert_eq!(line_bounds(s, 1), Some((3, 5, 6)));
        assert_eq!(line_bounds(s, 2), Some((6, 8, 8)));
        assert_eq!(line_bounds("", 0), None);
        let s2 = "one\n";
        assert_eq!(line_count(s2), 2);
        assert_eq!(line_bounds(s2, 1), Some((4, 4, 4)));
    }

    #[test]
    fn word_ranges() {
        assert_eq!(word_range_at("hello world", 1), Some((0, 5)));
        assert_eq!(word_range_at("hello world", 8), Some((6, 11)));
        assert_eq!(word_range_at("hello world", 5), None);
    }
}
