//! Document buffer model (spec §3.1–§3.5): one buffer = one file = one truth.
//!
//! Buffers are plain text with `\n` line endings internally; the original
//! line-ending style and BOM are preserved on save (spec F80/F81/F82).

use std::path::{Path, PathBuf};

/// Line-ending convention of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
}

impl Eol {
    /// Detect from raw file content (dominant style wins).
    pub fn detect(raw: &str) -> Eol {
        let crlf = raw.matches("\r\n").count();
        let lf = raw.matches('\n').count().saturating_sub(crlf);
        if crlf > lf {
            Eol::Crlf
        } else {
            Eol::Lf
        }
    }

    /// The platform default for new files.
    pub fn platform_default() -> Eol {
        if cfg!(windows) {
            Eol::Crlf
        } else {
            Eol::Lf
        }
    }

    /// Apply this convention to internal (LF-normalized) text.
    pub fn apply(&self, internal: &str) -> String {
        match self {
            Eol::Lf => internal.to_owned(),
            Eol::Crlf => internal.replace('\n', "\r\n"),
        }
    }
}

/// Snapshot of the on-disk state used for external-change detection (spec §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskStamp {
    pub len: u64,
    /// Seconds since UNIX_EPOCH with sub-second precision lost is fine; we only
    /// need *change* detection. Stored as millis for simplicity.
    pub modified_millis: Option<u128>,
}

impl DiskStamp {
    pub fn from_meta(meta: &std::fs::Metadata) -> Self {
        let modified_millis = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis());
        Self { len: meta.len(), modified_millis }
    }

    pub fn changed_vs(&self, other: &DiskStamp) -> bool {
        self.len != other.len || self.modified_millis != other.modified_millis
    }
}

/// Encoding provenance of the loaded text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    /// Decoded with lossy replacement — a banner must warn the user (F83).
    Lossy,
}

impl Encoding {
    pub fn label(&self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 BOM",
            Encoding::Utf16Le => "UTF-16 LE",
            Encoding::Utf16Be => "UTF-16 BE",
            Encoding::Lossy => "repaired",
        }
    }
}

/// A single open document (spec §1 principle 1).
pub struct Buffer {
    pub id: u64,
    /// `None` until the first Save As (untitled buffer).
    pub path: Option<PathBuf>,
    /// Internal text — always `\n` line endings.
    pub text: String,
    /// Revision counter, incremented on every mutation.
    pub revision: u64,
    /// Revision at last save/load boundary → dirty detection (O(1)).
    pub saved_revision: u64,
    pub eol: Eol,
    pub encoding: Encoding,
    pub read_only: bool,
    /// File deleted/moved while open (F86).
    pub missing_on_disk: bool,
    pub disk_stamp: Option<DiskStamp>,
    /// Set when we chose to keep our edits over a changed file; clears next save.
    pub suppress_external_change_once: bool,
    /// Optional display-name override (recovery buffers).
    pub title_override: Option<String>,
    /// Cached O(n) statistics, valid while `stats_rev == revision`.
    stats_rev: u64,
    words: usize,
    lines: usize,
}

impl Buffer {
    /// Recompute the cached word/line counts, but only when the text changed
    /// since the last call. O(1) per frame instead of O(document) per frame.
    pub fn refresh_stats(&mut self) {
        if self.stats_rev != self.revision {
            self.words = self.text.split_whitespace().count();
            self.lines = if self.text.is_empty() { 0 } else { self.text.bytes().filter(|&b| b == b'\n').count() + 1 };
            self.stats_rev = self.revision;
        }
    }
}

impl Buffer {
    pub fn untitled(id: u64) -> Self {
        Self {
            id,
            path: None,
            text: String::new(),
            revision: 0,
            saved_revision: 0,
            eol: Eol::platform_default(),
            encoding: Encoding::Utf8,
            read_only: false,
            missing_on_disk: false,
            disk_stamp: None,
            suppress_external_change_once: false,
            title_override: None,
            stats_rev: u64::MAX,
            words: 0,
            lines: 0,
        }
    }

    pub fn from_disk(id: u64, path: &Path, text: String, encoding: Encoding, read_only: bool, stamp: DiskStamp) -> Self {
        let eol = Eol::detect(&text);
        // Normalize to LF internally.
        let text = text.replace("\r\n", "\n");
        Self {
            id,
            path: Some(path.to_owned()),
            text,
            revision: 0,
            saved_revision: 0,
            eol,
            encoding,
            read_only,
            missing_on_disk: false,
            disk_stamp: Some(stamp),
            suppress_external_change_once: false,
            title_override: None,
            stats_rev: u64::MAX,
            words: 0,
            lines: 0,
        }
    }

    pub fn title(&self) -> String {
        if let Some(t) = &self.title_override {
            return t.clone();
        }
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("Untitled-{}", self.id))
    }

    pub fn path_or_title(&self) -> String {
        self.path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| self.title())
    }

    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    /// Cached when [`Buffer::refresh_stats`] has run for the current revision,
    /// otherwise computed on the fly so the value is never stale.
    pub fn word_count(&self) -> usize {
        if self.stats_rev == self.revision {
            self.words
        } else {
            self.text.split_whitespace().count()
        }
    }

    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Number of lines in the buffer text.
    pub fn line_count(&self) -> usize {
        if self.stats_rev == self.revision {
            self.lines
        } else {
            if self.text.is_empty() {
                0
            } else {
                self.text.bytes().filter(|&b| b == b'\n').count() + 1
            }
        }
    }

    /// Mark saved: called only after a successful write/load.
    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
        self.suppress_external_change_once = false;
        if let Some(path) = self.path.clone() {
            if let Ok(meta) = std::fs::metadata(&path) {
                self.disk_stamp = Some(DiskStamp::from_meta(&meta));
                self.missing_on_disk = false;
            }
        }
    }

    /// The text to write for this buffer under its current encoding settings.
    pub fn encoded_bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        match self.encoding {
            Encoding::Utf8Bom => out.extend_from_slice(&[0xEF, 0xBB, 0xBF]),
            Encoding::Utf16Le => {
                // Internal text is UTF-8; convert for round-trip preservation.
                for u in self.eol.apply(&self.text).encode_utf16() {
                    out.extend_from_slice(&u.to_le_bytes());
                }
                return out;
            }
            Encoding::Utf16Be => {
                for u in self.eol.apply(&self.text).encode_utf16() {
                    out.extend_from_slice(&u.to_be_bytes());
                }
                return out;
            }
            Encoding::Utf8 | Encoding::Lossy => {}
        }
        // Fall-through: every non-UTF-16 encoding still writes the body, after an
        // optional BOM prefix. Never drop this — it is the document itself.
        out.extend_from_slice(self.eol.apply(&self.text).as_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eol_detect() {
        assert_eq!(Eol::detect("a\nb\n"), Eol::Lf);
        assert_eq!(Eol::detect("a\r\nb\r\n"), Eol::Crlf);
        assert_eq!(Eol::detect("a\r\nb\nc\n"), Eol::Lf);
    }

    #[test]
    fn dirty_lifecycle() {
        let mut b = Buffer::untitled(1);
        assert!(!b.is_dirty());
        b.text.push_str("hi");
        b.revision += 1;
        assert!(b.is_dirty());
        b.mark_saved();
        assert!(!b.is_dirty());
    }

    #[test]
    fn crlf_roundtrip() {
        let text = "line1\r\nline2\r\n";
        let b = Buffer::from_disk(1, Path::new("x.md"), text.to_owned(), Encoding::Utf8Bom, false, DiskStamp { len: 0, modified_millis: None });
        assert_eq!(b.eol, Eol::Crlf);
        assert_eq!(b.text, "line1\nline2\n");
        // BOM round-trips: re-encoded bytes carry the BOM prefix + CRLF endings.
        let mut expected = vec![0xEF, 0xBB, 0xBF];
        expected.extend_from_slice(text.as_bytes());
        assert_eq!(b.encoded_bytes(), expected);
    }

    #[test]
    fn stats_match_uncached_reference() {
        let mut b = Buffer::untitled(7);
        b.text = "alpha beta\ngamma\n\n  delta  ".to_owned();
        // Before refresh_stats the values are still exact (computed on demand).
        assert_eq!(b.word_count(), 4);
        assert_eq!(b.line_count(), 4);

        b.refresh_stats();
        assert_eq!(b.word_count(), 4);
        assert_eq!(b.line_count(), 4);
    }

    #[test]
    fn stats_invalidate_on_revision_bump() {
        let mut b = Buffer::untitled(7);
        b.text = "one two three".to_owned();
        b.revision = 1;
        b.refresh_stats();
        assert_eq!(b.word_count(), 3);
        assert_eq!(b.line_count(), 1);

        // Mutate + bump: a refreshed cache must never serve the old counts.
        b.text.push_str("\nfour\nfive six");
        b.revision = 2;
        assert_eq!(b.word_count(), 6);
        assert_eq!(b.line_count(), 3);
        b.refresh_stats();
        assert_eq!(b.word_count(), 6);
        assert_eq!(b.line_count(), 3);
    }

    #[test]
    fn stats_on_empty_buffer() {
        let mut b = Buffer::untitled(1);
        assert_eq!(b.word_count(), 0);
        assert_eq!(b.line_count(), 0);
        b.refresh_stats();
        assert_eq!(b.word_count(), 0);
        assert_eq!(b.line_count(), 0);
    }

    #[test]
    fn utf16_roundtrip_writes_body() {
        let text = "line1\r\nline2\r\n";
        for (enc, le) in [(Encoding::Utf16Le, true), (Encoding::Utf16Be, false)] {
            let b = Buffer::from_disk(1, Path::new("x.md"), text.to_owned(), enc, false, DiskStamp { len: 0, modified_millis: None });
            let bytes = b.encoded_bytes();
            // 14 chars after CRLF re-application -> 2 bytes each, never empty.
            assert_eq!(bytes.len(), 28, "{enc:?} lost the document body");
            let mut u16s: Vec<u16> = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let pair = [chunk[0], chunk[1]];
                u16s.push(if le { u16::from_le_bytes(pair) } else { u16::from_be_bytes(pair) });
            }
            assert_eq!(String::from_utf16(&u16s).expect("valid utf16"), text);
        }
    }

    #[test]
    fn plain_utf8_writes_body_without_bom() {
        let text = "hello\nworld\n";
        let b = Buffer::from_disk(1, Path::new("x.md"), text.to_owned(), Encoding::Utf8, false, DiskStamp { len: 0, modified_millis: None });
        assert_eq!(b.encoded_bytes(), text.as_bytes());
    }

    #[test]
    fn refresh_stats_is_idempotent() {
        let mut b = Buffer::untitled(1);
        b.text = "a b c".to_owned();
        b.revision = 1;
        b.refresh_stats();
        let (w, l) = (b.word_count(), b.line_count());
        for _ in 0..5 {
            b.refresh_stats();
            assert_eq!((b.word_count(), b.line_count()), (w, l));
        }
    }
}