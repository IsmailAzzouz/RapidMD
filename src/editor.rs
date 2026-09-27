//! Editor rendering helpers: markdown-colored `LayoutJob` plus find-match
//! highlighting (spec F42/F49) and caret metrics.

use eframe::egui::{self, Color32, FontId};
use eframe::egui::text::{LayoutJob, TextFormat};
use std::sync::Arc;

use crate::highlight::Tk;
use crate::theme::Palette;

/// Reusable scratch space for one editor repaint.
///
/// The layouter runs on every frame, and on a large document it produces
/// hundreds of thousands of sections. The boundary vector is the largest
/// recyclable allocation, so callers that repaint repeatedly hand the same
/// scratch back each time instead of letting a fresh one be allocated.
#[derive(Default)]
pub struct JobScratch {
    points: Vec<usize>,
}

/// Syntax tokens cached against the (buffer id, revision) they were produced
/// from. Re-tokenizing the whole document on every repaint is wasted work: the
/// token list only changes when the text changes.
#[derive(Default)]
pub struct TokenCache {
    key: Option<(u64, u64)>,
    tokens: Vec<(std::ops::Range<usize>, Tk)>,
}

impl TokenCache {
    /// Tokens for `text`, re-tokenizing only if `key` differs from the cached one.
    pub fn tokens_for(&mut self, text: &str, key: (u64, u64)) -> &[(std::ops::Range<usize>, Tk)] {
        if self.key != Some(key) {
            self.tokens.clear();
            crate::highlight::tokenize_into(text, &mut self.tokens);
            self.key = Some(key);
        }
        &self.tokens
    }
}

/// Single-section job for documents too large to syntax-color.
///
/// Same output as `egui::text::LayoutJob::simple`, but appends the borrowed
/// slice directly instead of first copying the whole text into a fresh
/// `String` — on a 10 MiB document that copy was the largest per-frame
/// allocation in the editor.
pub fn plain_job(text: &str, font: &FontId, color: Color32, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.append(
        text,
        0.0,
        TextFormat { font_id: font.clone(), color, ..Default::default() },
    );
    job
}

/// Like `editor_job_into`, but skips tokenization while the cached tokens are
/// still valid for `key` (buffer id, revision). Allocates its own scratch.
pub fn editor_job_cached(
    text: &str,
    pal: &Palette,
    font: &FontId,
    matches: &[std::ops::Range<usize>],
    cache: &std::cell::RefCell<TokenCache>,
    key: (u64, u64),
) -> LayoutJob {
    editor_job_cached_with(text, pal, font, matches, cache, key, &mut JobScratch::default())
}

/// [`editor_job_cached`] against caller-owned scratch space, so a repaint loop
/// keeps one boundary vector alive across frames.
pub fn editor_job_cached_with(
    text: &str,
    pal: &Palette,
    font: &FontId,
    matches: &[std::ops::Range<usize>],
    cache: &std::cell::RefCell<TokenCache>,
    key: (u64, u64),
    scratch: &mut JobScratch,
) -> LayoutJob {
    let mut cache = cache.borrow_mut();
    let tokens = cache.tokens_for(text, key);
    // `LayoutJob` is consumed by `Fonts::layout_job`, so it cannot itself be
    // recycled; the boundary vector can, and at ~2 x section count it is the
    // bigger of the two allocations on a large document.
    let mut job = LayoutJob::default();
    job_from_tokens_into(text, tokens, pal, font, matches, &mut scratch.points, &mut job);
    job
}

/// Merge token colors with find-match backgrounds into a single layout job.
///
/// The caller owns `token_buf` so the token vector is reused across repaints
/// instead of being reallocated for every frame.
pub fn editor_job_into(
    text: &str,
    pal: &Palette,
    font: &FontId,
    matches: &[std::ops::Range<usize>],
    token_buf: &mut Vec<(std::ops::Range<usize>, Tk)>,
) -> LayoutJob {
    crate::highlight::tokenize_into(text, token_buf);
    job_from_tokens(text, token_buf, pal, font, matches)
}

/// Build the layout job from already-tokenized text.
pub fn job_from_tokens(
    text: &str,
    tokens: &[(std::ops::Range<usize>, Tk)],
    pal: &Palette,
    font: &FontId,
    matches: &[std::ops::Range<usize>],
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job_from_tokens_into(text, tokens, pal, font, matches, &mut Vec::new(), &mut job);
    job
}

/// Built layout job, kept across repaints so an unchanged document costs one
/// `clone` instead of a full re-tokenize-and-append pass.
pub struct JobCache {
    key: Option<JobKey>,
    job: Option<LayoutJob>,
}
/// Which builder produced the retained job. Syntax-colored and plain jobs
/// differ, so switching a document across `MAX_SYNTAX_CHARS` must invalidate.
#[derive(PartialEq, Eq, Clone, Copy)]
enum JobKind {
    Plain,
    Syntax,
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct JobKey {
    kind: JobKind,
    buf: u64,
    rev: u64,
    /// Identity of the find-match set: the `Arc` the layouter was handed, so a
    /// new search invalidates and an unchanged one does not.
    matches: usize,
    matches_len: usize,
    color: Color32,
    font_bits: u32,
    wrap_bits: u32,
}

impl Default for JobCache {
    fn default() -> Self {
        Self { key: None, job: None }
    }
}

impl JobCache {
    /// The layout job for `text`, rebuilt only when one of the inputs changed.
    ///
    /// Rebuilding is O(document) — on a 10 MiB file it is ~70 ms of token
    /// boundary sorting and section appends, paid on *every* repaint, including
    /// the ones caused by nothing more than a mouse move. Cloning the retained
    /// job is ~17 ms for the same document, so an unchanged editor repaint
    /// costs roughly a quarter as much.
    pub fn job(
        &mut self,
        text: &str,
        pal: &Palette,
        font: &FontId,
        matches: &[std::ops::Range<usize>],
        matches_id: usize,
        key: (u64, u64),
        wrap_width: f32,
        scratch: &mut JobScratch,
    ) -> LayoutJob {
        let k = JobKey {
            kind: JobKind::Syntax,
            buf: key.0,
            rev: key.1,
            matches: matches_id,
            matches_len: matches.len(),
            color: pal.text,
            font_bits: font.size.to_bits(),
            wrap_bits: wrap_width.to_bits(),
        };
        // The text length is part of the identity as a backstop: two buffers can
        // share a (buf, revision) pair across a file swap only if the caller
        // keeps `buf` unique per document, which `Buffer::id` guarantees.
        if self.key != Some(k) || self.job.as_ref().is_none_or(|j| j.text.len() != text.len()) {
            let mut job = LayoutJob::default();
            {
                let mut cache = TokenCache::default();
                let tokens = cache.tokens_for(text, key);
                job_from_tokens_into(text, tokens, pal, font, matches, &mut scratch.points, &mut job);
            }
            job.wrap.max_width = wrap_width;
            self.job = Some(job);
            self.key = Some(k);
        }
        // `layout_job` consumes the job by value, so the retained copy is
        // cloned out rather than moved.
        self.job.as_ref().expect("just populated").clone()
    }

    /// Single-section job for documents past the syntax-coloring threshold.
    ///
    /// Cached on the same inputs as [`JobCache::job`], because a document over
    /// `MAX_SYNTAX_CHARS` still repaints on every frame and a rebuild here is
    /// one full-buffer `String` copy.
    pub fn plain(
        &mut self,
        text: &str,
        font: &FontId,
        color: Color32,
        wrap_width: f32,
        key: (u64, u64),
    ) -> LayoutJob {
        let k = JobKey {
            kind: JobKind::Plain,
            buf: key.0,
            rev: key.1,
            matches: 0,
            matches_len: 0,
            color,
            font_bits: font.size.to_bits(),
            wrap_bits: wrap_width.to_bits(),
        };
        if self.key != Some(k) || self.job.as_ref().is_none_or(|j| j.text.len() != text.len()) {
            let mut job = plain_job(text, font, color, wrap_width);
            job.text.reserve(text.len());
            self.job = Some(job);
            self.key = Some(k);
        }
        self.job.as_ref().expect("just populated").clone()
    }
}

fn job_from_tokens_into(
    text: &str,
    tokens: &[(std::ops::Range<usize>, Tk)],
    pal: &Palette,
    font: &FontId,
    matches: &[std::ops::Range<usize>],
    points: &mut Vec<usize>,
    job: &mut LayoutJob,
) {
    // Boundaries = token starts/ends ∪ match starts/ends.
    points.clear();
    points.reserve(tokens.len() * 2 + matches.len() * 2 + 2);
    for (r, _) in tokens.iter() {
        points.push(r.start);
        points.push(r.end);
    }
    for m in matches {
        points.push(m.start);
        points.push(m.end);
    }
    points.push(0);
    points.push(text.len());
    points.sort_unstable();
    points.dedup();

    // `LayoutJob::append` pushes into `sections` and appends to `text`. Both
    // grow to document scale (on a 10 MiB file, ~700k sections and 10 MiB of
    // text), and growing a `Vec` that large one doubling at a time copies
    // gigabytes before the job is complete. Reserving up front removes those
    // reallocation copies. `append` merges same-format neighbours, so the
    // section count is only an upper bound; the reservation is not exact.
    job.sections.reserve(points.len().saturating_sub(1));
    job.text.reserve(text.len());

    let color_of = |tk: Tk, pal: &Palette| -> Color32 {
        match tk {
            Tk::Plain => pal.text,
            Tk::Heading => pal.md_heading,
            Tk::Strong => pal.md_strong,
            Tk::Emphasis => pal.md_em,
            Tk::Strike => pal.faint,
            Tk::Code => pal.md_url,
            Tk::LinkText => pal.md_link,
            Tk::LinkUrl => pal.md_url,
            Tk::ListMarker => pal.md_list,
            Tk::Quote => pal.md_quote,
            Tk::Task => pal.md_task,
            Tk::Hr => pal.md_hr,
            Tk::Html => pal.md_html,
            Tk::Fence => pal.md_fence,
            Tk::CodeText => pal.code_text,
        }
    };

    job.break_on_newline = true;
    // Hoisted out of the loop: `bold_font` / the `FontFamily::Name` built from
    // a `&'static str` each allocate a `String`, and this loop runs once per
    // section (hundreds of thousands on a large document). Likewise the match
    // scan, which was an O(matches) `iter().any` per segment.
    let bold = crate::theme::bold_font(font.size);

    // Tokens and points are both in ascending document order, so a single
    // forward-only cursor replaces the previous per-segment linear search
    // (which made the editor layouter quadratic in token count).
    let mut cursor = 0usize;
    // Forward-only cursor over the match list as well.
    let mut mcur = 0usize;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a == b {
            continue;
        }
        let seg = &text[a..b];
        if seg.is_empty() {
            continue;
        }
        // Determine dominant token kind at this segment's start.
        while cursor < tokens.len() && tokens[cursor].0.end <= a {
            cursor += 1;
        }
        let kind = match tokens.get(cursor) {
            Some((r, k)) if r.start <= a && a < r.end => *k,
            _ => Tk::Plain,
        };
        let mut color = color_of(kind, pal);
        let mut bg = if kind == Tk::Code || kind == Tk::CodeText { pal.code_bg } else { Color32::TRANSPARENT };
        // Find-match highlight overrides. Both lists are ascending, so a
        // forward cursor answers "is `a` inside any match" in amortized O(1).
        if !matches.is_empty() {
            while mcur < matches.len() && matches[mcur].end <= a {
                mcur += 1;
            }
            if mcur < matches.len() && matches[mcur].start <= a {
                bg = pal.match_bg;
                color = pal.text;
            }
        }
        // Highlight the markdown itself: `**strong**` in a bold face, `*em*` skewed.
        let fmt = TextFormat {
            font_id: if kind == Tk::Strong { bold.clone() } else { font.clone() },
            color,
            background: bg,
            italics: kind == Tk::Emphasis,
            ..Default::default()
        };
        job.append(seg, 0.0, fmt);
    }
}

/// Line/column of a byte offset (1-based numbers for the status bar).
pub fn line_col(text: &str, byte: usize) -> (usize, usize) {
    let before = &text[..byte.min(text.len())];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let ls = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = text[ls..byte.min(text.len())].chars().count() + 1;
    (line, col)
}

/// Convenience wrapper so callers can keep matches in an `Arc` for the layouter.
/// The token buffer is owned by the closure and reused on every invocation.
pub fn find_layouter<'a>(
    pal: Palette,
    font: FontId,
    matches: Arc<Vec<std::ops::Range<usize>>>,
) -> impl FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> Arc<egui::Galley> + 'a {
    let mut token_buf = Vec::new();
    move |ui, buffer, wrap_width| {
        let mut job = editor_job_into(buffer.as_str(), &pal, &font, &matches, &mut token_buf);
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|f| f.layout_job(job))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_basics() {
        assert_eq!(line_col("abc", 0), (1, 1));
        assert_eq!(line_col("abc\ndef", 4), (2, 1));
        assert_eq!(line_col("abc\ndef", 6), (2, 3));
    }

    #[test]
    fn token_cache_reuses_tokens_for_same_key() {
        let mut cache = TokenCache::default();
        let text = "# Title\n\nsome **bold** text\n";
        let first = cache.tokens_for(text, (1, 7)).len();
        let ptr = cache.tokens.as_ptr();
        // Same key: no re-tokenization, same allocation.
        let second = cache.tokens_for(text, (1, 7)).len();
        assert_eq!(first, second);
        assert_eq!(cache.tokens.as_ptr(), ptr, "cache must reuse its allocation");
        // New revision: re-tokenized, but still reusing the same allocation.
        let (has_emphasis, count) = {
            let changed = cache.tokens_for("# Title\n\nsome *italic* text\n", (1, 8));
            (changed.iter().any(|(_, k)| *k == Tk::Emphasis), changed.len())
        };
        assert!(count > 0);
        assert_eq!(cache.tokens.as_ptr(), ptr, "re-tokenization must reuse the allocation");
        assert!(has_emphasis, "new text must be re-tokenized");
    }

    #[test]
    fn plain_job_matches_layout_job_simple() {
        let pal = Palette::dark();
        let font = FontId::monospace(14.0);
        let text = "line one\nline **two**\nline three\n";
        let mine = plain_job(text, &font, pal.text, 300.0);
        let theirs = egui::text::LayoutJob::simple(text.to_owned(), font.clone(), pal.text, 300.0);
        assert_eq!(mine.sections.len(), theirs.sections.len());
        assert_eq!(mine.text, text, "no extra copy, and identical text");
        assert_eq!(mine.text, theirs.text);
        assert_eq!(mine.sections[0].byte_range, theirs.sections[0].byte_range);
        assert_eq!(mine.wrap.max_width, theirs.wrap.max_width);
    }

    #[test]
    fn scratch_reuse_does_not_change_the_job() {
        let pal = Palette::dark();
        let font = FontId::monospace(14.0);
        let text = "# H\n\nsome **bold** and *em* text\n\n```rust\nlet x = 1;\n```\n";
        let cache = std::cell::RefCell::new(TokenCache::default());
        let mut scratch = JobScratch::default();
        let a = editor_job_cached_with(text, &pal, &font, &[], &cache, (1, 1), &mut scratch);
        let b = editor_job_cached_with(text, &pal, &font, &[], &cache, (1, 1), &mut scratch);
        assert_eq!(a.sections.len(), b.sections.len());
        assert_eq!(a.text, b.text);
        let dump = |j: &LayoutJob| {
            j.sections
                .iter()
.map(|s| (s.byte_range.clone(), s.format.color, s.format.background, s.format.italics))
                .collect::<Vec<_>>()
        };
        assert_eq!(dump(&a), dump(&b));
        assert_eq!(dump(&a), dump(&b));
    }

    /// The job cache must be transparent: every input that can change the output
    /// has to produce a rebuilt job matching the uncached build exactly.
    #[test]
    fn job_cache_matches_uncached_for_every_key() {
        let pal_dark = Palette::dark();
        let pal_light = Palette::light();
        let font = FontId::monospace(14.0);
        let text = "# H\n\nsome **bold** and *em* text with `code`\n\n- [x] done\n\n> quote\n";
        let dump = |j: &LayoutJob| {
            j.sections
                .iter()
                .map(|s| (s.byte_range.clone(), s.format.color, s.format.background, s.format.italics))
                .collect::<Vec<_>>()
        };
        let direct = |pal: &Palette, matches: &[std::ops::Range<usize>], wrap: f32| {
            let syn = crate::highlight::tokenize(text);
            let mut j = job_from_tokens(text, &syn.tokens, pal, &font, matches);
            j.wrap.max_width = wrap;
            j
        };
let cases: Vec<(&Palette, Vec<std::ops::Range<usize>>, f32, usize, (u64, u64))> = vec![
            (&pal_dark, vec![], 400.0, 1, (1, 1)),
            (&pal_dark, vec![], 400.0, 2, (1, 1)),
            (&pal_dark, vec![], 600.0, 3, (1, 1)),
            (&pal_light, vec![], 400.0, 4, (1, 1)),
            (&pal_dark, vec![5..12], 400.0, 5, (1, 1)),
            (&pal_dark, vec![], 400.0, 6, (1, 2)),
            (&pal_dark, vec![], 400.0, 7, (2, 1)),
        ];
        for (pal, matches, wrap, matches_id, key) in cases {
            let mut cache = JobCache::default();
            let mut scratch = JobScratch::default();
            let got = cache.job(text, pal, &font, &matches, matches_id, key, wrap, &mut scratch);
            let want = direct(pal, &matches, wrap);
            assert_eq!(got.text, want.text, "text differs for matches_id {matches_id}");
            assert_eq!(dump(&got), dump(&want), "sections differ for matches_id {matches_id}");
            assert_eq!(got.wrap.max_width, want.wrap.max_width);
        }
    }

    /// A revision bump must rebuild, and a different buffer id must rebuild too.
    ///
    /// Note the contract: `(buf_id, revision)` is authoritative. A same-length
    /// edit that did *not* bump the revision would serve stale text — which is
    /// exactly why every `Buffer` mutation increments the counter. The text
    /// length check in `JobCache` is a cheap backstop, not the mechanism.
    #[test]
    fn job_cache_rebuilds_on_revision_bump() {
        let pal = Palette::dark();
        let font = FontId::monospace(14.0);
        let mut cache = JobCache::default();
        let mut scratch = JobScratch::default();

        let a = "hello world".to_owned();
        let j = cache.job(&a, &pal, &font, &[], 1, (1, 1), 400.0, &mut scratch);
        assert_eq!(j.text, a);

        // Revision bumped: must reflect the edit, even at identical length.
        let b = "HELLO WORLD".to_owned();
        let j2 = cache.job(&b, &pal, &font, &[], 1, (1, 2), 400.0, &mut scratch);
        assert_eq!(j2.text, b, "a revision bump must rebuild the job");

        // Unchanged key and length: served from cache, still correct.
        let j3 = cache.job(&b, &pal, &font, &[], 1, (1, 2), 400.0, &mut scratch);
        assert_eq!(j3.text, b);

        // A different buffer: distinct key, so a rebuild.
        let c = "different buffer text".to_owned();
        let j4 = cache.job(&c, &pal, &font, &[], 1, (7, 1), 400.0, &mut scratch);
        assert_eq!(j4.text, c, "a different buffer id must rebuild the job");
    }

    /// The plain (past `MAX_SYNTAX_CHARS`) path is cached on the same contract
    /// and must never be served a syntax-colored job, or vice versa.
    #[test]
    fn plain_cache_matches_uncached_and_switches_kind() {
        let pal = Palette::dark();
        let font = FontId::monospace(14.0);
        let text = "# H\n\nplain body text\n";
        let mut cache = JobCache::default();
        let mut scratch = JobScratch::default();

        let p = cache.plain(text, &font, pal.text, 400.0, (1, 1));
        assert_eq!(p.text, text);
        assert_eq!(p.sections.len(), 1, "plain job is a single section");
        assert_eq!(p.sections[0].format.color, pal.text);
        assert_eq!(p.wrap.max_width, 400.0);

        // Repeat: served from cache, identical.
        let p2 = cache.plain(text, &font, pal.text, 400.0, (1, 1));
        assert_eq!(p2.text, p.text);
        assert_eq!(p2.sections.len(), p.sections.len());

        // Switching to the syntax path for the same buffer must invalidate:
        // a heading here is colored differently from body text.
        let s = cache.job(text, &pal, &font, &[], 1, (1, 1), 400.0, &mut scratch);
        assert_eq!(s.text, text);
        assert_ne!(
            s.sections.len(),
            1,
            "syntax job must be rebuilt, not served the cached plain one"
        );

        // And back again.
        let p3 = cache.plain(text, &font, pal.text, 400.0, (1, 1));
        assert_eq!(p3.sections.len(), 1, "plain job must be rebuilt after a syntax job");

        // Different width: rebuild with the new wrap width.
        let p4 = cache.plain(text, &font, pal.text, 900.0, (1, 1));
        assert_eq!(p4.wrap.max_width, 900.0);
    }
}
