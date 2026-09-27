//! Before/after benchmark: identical logic compiled against the pre-optimization
//! tree and the current tree, so the numbers are directly comparable.
//!
//! Run from each tree root. Peak working set is read from the OS via std only,
//! so the baseline manifest needs no bench-only dependency.
//!
//! Only the editor-job call differs between the two variants (the function was
//! renamed during optimization); everything else is byte-identical.

use std::time::Instant;

use rustdown_viewer::highlight;
use rustdown_viewer::md;
use rustdown_viewer::theme::Palette;

/// Deterministic corpus: no RNG, no I/O, identical bytes in every run.
fn corpus(mib: usize) -> String {
    let target = mib * 1024 * 1024;
    let unit = "# Chapter\n\nSome **bold** and _italic_ prose with `code` and a [link](https://example.com).\n\n\
- item one\n- item two with ~~strike~~\n- [ ] a task\n\n> a quote\n\n\
```rust\nfn main() { println!(\"hi\"); }\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n";
    let mut s = String::with_capacity(target + unit.len());
    while s.len() < target {
        s.push_str(unit);
    }
    s.truncate(target);
    s
}

fn main() {
    let pal = Palette::dark();
    let font = eframe::egui::FontId::monospace(13.5);
    let no_matches: Vec<std::ops::Range<usize>> = Vec::new();

    println!("RapidMD before/after benchmark (headless)");
    println!("  (peak working set is measured externally by tools/run_and_measure.ps1)");

    // Sizes come from argv so a slow (pre-optimization) tree can be measured
    // document-size by document-size instead of timing out on the largest.
    let sizes: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse::<usize>().ok())
        .filter(|m| *m > 0)
        .collect();
    let sizes = if sizes.is_empty() { vec![1usize, 10, 100] } else { sizes };

    for mib in sizes {
        println!();
        println!("  -- {} MiB document --", mib);

        let t = Instant::now();
        let text = corpus(mib);
        let gen_ms = t.elapsed().as_secs_f64() * 1e3;
        println!(
            "     generate             {:8.1} ms  ({} bytes)",
            gen_ms,
            text.len()
        );

        let t = Instant::now();
        let doc = md::parse(&text);
        let parse_ms = t.elapsed().as_secs_f64() * 1e3;
        println!(
            "     md::parse            {:8.1} ms  ({} blocks)",
            parse_ms,
            doc.blocks.len()
        );
        drop(doc);

        // Both trees expose `highlight::tokenize` with the same signature, so the
        // tokenizer is compared through the identical entry point.
        let t = Instant::now();
        let syn = highlight::tokenize(&text);
        let tok_ms = t.elapsed().as_secs_f64() * 1e3;
        let ntok = syn.tokens.len();
        drop(syn);
        println!("     highlight::tokenize  {:8.1} ms  ({} tokens)", tok_ms, ntok);

        #[cfg(feature = "baseline")]
        {
            // Baseline `editor_job` takes `FontId` by value; build a fresh one
            // so the shared `font` binding is not moved.
            let font = eframe::egui::FontId::monospace(13.5);
            let t = Instant::now();
            let job = rustdown_viewer::editor::editor_job(&text, &pal, font, &no_matches);
            let job_ms = t.elapsed().as_secs_f64() * 1e3;
            println!(
                "     editor_job           {:8.1} ms  ({} sections)",
                job_ms,
                job.sections.len()
            );
            drop(job);
        }
        #[cfg(not(feature = "baseline"))]
        {
            let mut token_buf: Vec<(std::ops::Range<usize>, highlight::Tk)> = Vec::new();
            let t = Instant::now();
            let job = rustdown_viewer::editor::editor_job_into(
                &text,
                &pal,
                &font,
                &no_matches,
                &mut token_buf,
            );
            let job_ms = t.elapsed().as_secs_f64() * 1e3;
            println!(
                "     editor_job_into      {:8.1} ms  ({} sections)",
                job_ms,
                job.sections.len()
            );
            drop(job);
        }
    }

    println!();
}
