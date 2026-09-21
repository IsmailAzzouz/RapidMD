//! Editor rendering helpers: markdown-colored `LayoutJob` plus find-match
//! highlighting (spec F42/F49) and caret metrics.

use eframe::egui::{self, Color32, FontId};
use eframe::egui::text::{LayoutJob, TextFormat};
use std::sync::Arc;

use crate::highlight::{Tk, tokenize};
use crate::theme::Palette;

/// Merge token colors with find-match backgrounds into a single layout job.
pub fn editor_job(text: &str, pal: &Palette, font: FontId, matches: &[std::ops::Range<usize>]) -> LayoutJob {
    let syn = tokenize(text);
    let tokens = syn.tokens;
    // Boundaries = token starts/ends ∪ match starts/ends.
    let mut points: Vec<usize> = Vec::with_capacity(tokens.len() * 2 + matches.len() * 2);
    for (r, _) in &tokens {
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

    let mut job = LayoutJob::default();
    job.break_on_newline = true;
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
        let kind = tokens
            .iter()
            .find(|(r, _)| r.start <= a && a < r.end)
            .map(|(_, k)| *k)
            .unwrap_or(Tk::Plain);
        let mut color = color_of(kind, pal);
        let mut bg = Color32::TRANSPARENT;
        if kind == Tk::Code || kind == Tk::CodeText {
            bg = pal.code_bg;
        }
        // Find-match highlight overrides.
        if matches.iter().any(|m| m.start <= a && a < m.end) {
            bg = pal.match_bg;
            color = pal.text;
        }
        // Highlight the markdown itself: `**strong**` in a bold face, `*em*` skewed.
        let font_id = if kind == Tk::Strong { crate::theme::bold_font(font.size) } else { font.clone() };
        let fmt = TextFormat {
            font_id,
            color,
            background: bg,
            italics: kind == Tk::Emphasis,
            ..Default::default()
        };
        job.append(seg, 0.0, fmt);
    }
    job
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
pub fn find_layouter<'a>(
    pal: Palette,
    font: FontId,
    matches: Arc<Vec<std::ops::Range<usize>>>,
) -> impl FnMut(&egui::Ui, &dyn egui::TextBuffer, f32) -> Arc<egui::Galley> + 'a {
    move |ui, buffer, wrap_width| {
        let mut job = editor_job(buffer.as_str(), &pal, font.clone(), &matches);
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

}
