//! Headless performance benchmark for RapidMD.
//!
//! Runs the real pipeline (markdown parse, editor tokenize, preview layout,
//! syntax highlight, image decode) with no window, and reports per-stage
//! timings plus the real process working set / peak working set.
//!
//! Run: `cargo run --profile fast --release-features --example perf -- [sizes_mb]`

use std::time::Instant;

use eframe::egui;
use rustdown_viewer::theme::Palette;
use rustdown_viewer::{editor, highlight, md, preview};

// ---------------------------------------------------------------------------
// Process memory
// ---------------------------------------------------------------------------

/// (current working set, peak working set) in bytes.
fn rss() -> (u64, u64) {
    #[cfg(windows)]
    {
        use std::mem::size_of;
        use windows_sys::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        unsafe {
            let size = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let mut ctr = PROCESS_MEMORY_COUNTERS { cb: size, ..std::mem::zeroed() };
            if GetProcessMemoryInfo(GetCurrentProcess(), &mut ctr, size) != 0 {
                (ctr.WorkingSetSize as u64, ctr.PeakWorkingSetSize as u64)
            } else {
                (0, 0)
            }
        }
    }
    #[cfg(not(windows))]
    {
        // VmRSS / VmHWM in kB.
        let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let get = |k: &str| -> u64 {
            s.lines()
                .find(|l| l.starts_with(k))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
                * 1024
        };
        (get("VmRSS:"), get("VmHWM:"))
    }
}

fn mib(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

fn rss_line(tag: &str) {
    let (cur, peak) = rss();
    println!("      rss {tag:<22} {:>8.1} MiB (peak {:>8.1} MiB)", mib(cur), mib(peak));
}

// ---------------------------------------------------------------------------
// Percentiles
// ---------------------------------------------------------------------------

struct Stats {
    samples: Vec<f64>,
}

impl Stats {
    fn new() -> Self {
        Self { samples: Vec::new() }
    }
    fn push(&mut self, ms: f64) {
        self.samples.push(ms);
    }
    fn report(&mut self, label: &str, unit: &str) {
        if self.samples.is_empty() {
            return;
        }
        self.samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = self.samples.len();
        let at = |q: f64| self.samples[((n as f64 * q) as usize).min(n - 1)];
        let total: f64 = self.samples.iter().sum();
        println!(
            "  {label:<34} n={n:<6} p50 {:>8.3} {unit}  p95 {:>8.3}  p99 {:>8.3}  max {:>8.3}  mean {:>8.3}",
            at(0.50),
            at(0.95),
            at(0.99),
            self.samples[n - 1],
            total / n as f64
        );
    }
}

// ---------------------------------------------------------------------------
// Synthetic document
// ---------------------------------------------------------------------------

const LANGS: &[&str] = &["rust", "python", "javascript", "c", "go", "json", "toml", "bash"];

/// Builds a realistic Markdown document of roughly `target_bytes`.
fn synth(target_bytes: usize) -> String {
    let mut s = String::with_capacity(target_bytes + 4096);
    let mut i = 0usize;
    while s.len() < target_bytes {
        let lang = LANGS[i % LANGS.len()];
        match i % 7 {
            0 => s.push_str(&format!("# Section {} Overview\n\n", i / 7)),
            1 => s.push_str(&format!(
                "Paragraph {} with **bold**, *italic*, `inline code`, ~~strike~~ and a \
                 [link](https://example.com/{i}) covering ordinary prose that is long enough \
                 to wrap on a normal width window. Lorem ipsum dolor sit amet.\n\n",
                i
            )),
            2 => s.push_str(&format!("- item {}\n- item {}\n  - nested {}\n\n", i, i + 1, i)),
            3 => s.push_str(&format!("```{}\nfn f_{i}() {{\n    let x = {};\n    println!(\"{{}}\", x);\n}}\n```\n\n", lang, i)),
            4 => s.push_str(&format!(
                "> Quoted material {i}. It should be indented and dimmed relative to body text.\n\n"
            )),
            5 => s.push_str(&format!(
                "| col a | col b | col c |\n| --- | --- | --- |\n| {i} | x | y |\n| {} | z | w |\n\n",
                i + 1
            )),
            _ => s.push_str("---\n\n"),
        }
        i += 1;
    }
    s
}

// ---------------------------------------------------------------------------
// Headless egui
// ---------------------------------------------------------------------------

fn headless_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(rustdown_viewer::app::build_base_fonts());
    ctx
}

fn raw_input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1200.0, 800.0))),
        ..Default::default()
    }
}

/// egui 0.36 drives a frame through `begin_pass` / `end_pass`.
fn pass<R>(ctx: &egui::Context, f: impl FnOnce(&egui::Context) -> R) -> R {
    ctx.begin_pass(raw_input());
    let out = f(ctx);
    let _ = ctx.end_pass();
    out
}

/// One full preview frame for `doc`, scrolled to `scroll_px`.
fn preview_frame(
    ctx: &egui::Context,
    doc: &md::Doc,
    pal: &Palette,
    images: &mut preview::ImageCache,
    heights: &mut preview::BlockHeights,
    hl: &mut preview::HighlightCache,
    key: (u64, u64),
    scroll_px: f32,
) {
    pass(ctx, |ctx| {
        // `CentralPanel` in egui 0.36 needs a parent `Ui`; a full-screen `Area`
        // gives the same 1200x800 root without a window.
        egui::Area::new(egui::Id::new("perf_root"))
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("perf")
                    .max_height(760.0)
                    .vertical_scroll_offset(scroll_px)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Tall content so the scroll offset is meaningful.
                        ui.set_height(200_000.0);
                        preview::render(ui, doc, pal, None, images, key, heights, hl);
                    });
            });
    });
}

// ---------------------------------------------------------------------------

fn bench_open(sizes_mb: &[usize]) {
    println!("\n=== 1. Open / parse / tokenize ===");
    for &mb in sizes_mb {
        println!("\n  -- {mb} MiB document --");
        let t = Instant::now();
        let text = synth(mb * 1024 * 1024);
        let gen_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("  generate corpus               {:>10.1} ms  ({} bytes)", gen_ms, text.len());
        rss_line("after corpus");

        let t = Instant::now();
        let doc = md::parse(&text);
        let parse_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("  md::parse                     {:>10.1} ms  ({} blocks)", parse_ms, doc.blocks.len());
        rss_line("after parse");

        let mut tokens = Vec::new();
        let t = Instant::now();
        highlight::tokenize_into(&text, &mut tokens);
        let tok_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("  highlight::tokenize_into      {:>10.1} ms  ({} tokens)", tok_ms, tokens.len());
        rss_line("after tokenize");

        // The full editor repaint work: tokenize once, then build the LayoutJob.
        let pal = Palette::dark();
        let font = egui::FontId::monospace(14.0);
        let mut tokens = Vec::new();
        let t = Instant::now();
        highlight::tokenize_into(&text, &mut tokens);
        let job = editor::editor_job_into(&text, &pal, &font, &[], &mut tokens);
        println!("  editor_job_into               {:>10.1} ms  ({} sections)", t.elapsed().as_secs_f64() * 1000.0, job.sections.len());
        rss_line("after editor job");
    }
}

fn bench_typing(mb: usize, keystrokes: usize) {
    println!("\n=== 2. Typing latency (repaint work per keystroke) ===");
    let mut text = synth(mb * 1024 * 1024);
    let pal = Palette::dark();
    let font = egui::FontId::monospace(14.0);
    let ctx = headless_ctx();
    pass(&ctx, |_| {});

    let mut cold = Stats::new(); // revision bumped: full re-tokenize + layout
    let mut warm = Stats::new(); // revision same: cached tokens

    // Deterministic pseudo-random insertion points.
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    let mut tokens = Vec::new();
    // Long-lived cache: first call tokenizes, every later call with the same
    // (buffer id, revision) reuses the token allocation.
    let cache = std::cell::RefCell::new(editor::TokenCache::default());
    let _ = editor::editor_job_cached(&text, &pal, &font, &[], &cache, (0, 1));

    for k in 0..keystrokes {
        let at = (next() as usize) % text.len().max(1);
        let at = (0..=at).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
        text.insert(at, 'x');

        let t = Instant::now();
        tokens.clear();
        highlight::tokenize_into(&text, &mut tokens);
        let _job = editor::editor_job_into(&text, &pal, &font, &[], &mut tokens);
        cold.push(t.elapsed().as_secs_f64() * 1000.0);

        // A repaint with no edit since the last one: the TokenCache makes this
        // a pure LayoutJob rebuild, no re-tokenization.
        if k % 25 == 0 {
            let t = Instant::now();
            let _job = editor::editor_job_cached(&text, &pal, &font, &[], &cache, (0, 1));
            warm.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    cold.report("cold (tokenize + build job)", "ms");
    warm.report("warm (cached tokens)", "ms");
    rss_line("after typing run");
}

fn bench_preview(mb: usize, frames: usize) {
    println!("\n=== 3. Preview frames (first paint, steady state, scrolled) ===");
    let text = synth(mb * 1024 * 1024);
    let pal = Palette::dark();
    let doc = md::parse(&text);
    let ctx = headless_ctx();
    let mut images = preview::ImageCache::default();
    let mut heights = preview::BlockHeights::default();
    let mut hl = preview::HighlightCache::default();

    // First frame: every visible code block must be highlighted for the first time.
    let mut first = Stats::new();
    let mut steady = Stats::new();
    let mut scrolled = Stats::new();

    for f in 0..frames {
        let t = Instant::now();
        preview_frame(&ctx, &doc, &pal, &mut images, &mut heights, &mut hl, (1, 1), 0.0);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        if f == 0 {
            first.push(ms);
            println!("  first frame                   {:>10.3} ms", ms);
        } else {
            steady.push(ms);
        }

        // Scrolling to a fresh offset forces new blocks into the visible band.
        let offset = 2000.0 * f as f32;
        let t = Instant::now();
        preview_frame(&ctx, &doc, &pal, &mut images, &mut heights, &mut hl, (1, 1), offset);
        scrolled.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    steady.report("steady frame (same offset)", "ms");
    scrolled.report("scrolled frame (new offset)", "ms");

    // A revision bump invalidates cached heights but not the highlight cache
    // (the code text is unchanged), so this isolates the height re-measure cost.
    let mut rebump = Stats::new();
    for f in 0..frames {
        let t = Instant::now();
        preview_frame(&ctx, &doc, &pal, &mut images, &mut heights, &mut hl, (1, 2 + f as u64), 0.0);
        rebump.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    rebump.report("revision-bumped frame", "ms");
    rss_line("after preview run");
}

fn bench_highlight() {
    println!("\n=== 4. Syntax highlight cache (code_job_cached) ===");
    let text = synth(64 * 1024);
    let code: String = {
        // Pull a fenced Rust block out of the corpus.
        let start = text.find("```rust").unwrap() + 7;
        let end = text[start..].find("```").unwrap() + start;
        text[start..end].to_owned()
    };
    let pal_dark = Palette::dark();
    let pal_light = Palette::light();
    let mut cache = preview::HighlightCache::default();
    let ctx = headless_ctx();
    pass(&ctx, |_| {});

    let _ = rustdown_viewer::preview::code_job_cached(&code, "rust", &pal_dark, 14.0, &mut cache);

    let mut warm = Stats::new();
    for _ in 0..2000 {
        let t = Instant::now();
        let _ = rustdown_viewer::preview::code_job_cached(&code, "rust", &pal_dark, 14.0, &mut cache);
        warm.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    warm.report("cache hit (same text/theme)", "ms");

    // Miss path: same content, new theme, so syntect must re-highlight.
    let mut miss = Stats::new();
    let flip = std::cell::Cell::new(false);
    for _ in 0..2000 {
        let dark = flip.get();
        flip.set(!dark);
        let t = Instant::now();
        let pal = if dark { &pal_dark } else { &pal_light };
        let _ = rustdown_viewer::preview::code_job_cached(&code, "rust", pal, 14.0, &mut cache);
        miss.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    miss.report("cache miss (theme flip)", "ms");
}

fn bench_images(dir: &std::path::Path, count: usize) {
    println!("\n=== 5. Image decode + ImageCache LRU ===");
    std::fs::create_dir_all(dir).unwrap();
    // Write `count` distinct PNGs once; the cache sees them as new paths.
    let mut paths = Vec::new();
    for i in 0..count {
        let p = dir.join(format!("bench_{i}.png"));
        if !p.exists() {
            let w = 512u32 + (i as u32 % 3) * 128;
            let h = 384u32 + (i as u32 % 5) * 64;
            let mut buf = image::RgbImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    buf.put_pixel(x, y, image::Rgb([(x % 256) as u8, (y % 256) as u8, (i as u8).wrapping_mul(7)]));
                }
            }
            buf.save(&p).unwrap();
        }
        paths.push(p);
    }
    rss_line("after writing corpus");

    let ctx = headless_ctx();
    pass(&ctx, |_| {});

    // Decode through the public cache path by rendering a doc of image blocks.
    let mut md_text = String::new();
    for p in &paths {
        md_text.push_str(&format!("![b]({})\n\n", p.display()));
    }
    let doc = md::parse(&md_text);
    let pal = Palette::dark();

    let t = Instant::now();
    let mut images = preview::ImageCache::default();
    let mut heights = preview::BlockHeights::default();
    let mut hl = preview::HighlightCache::default();
    preview_frame(&ctx, &doc, &pal, &mut images, &mut heights, &mut hl, (1, 1), 0.0);
    let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
    let (cur, peak) = rss();
    println!("  cold decode of {count} images   {:>10.1} ms", cold_ms);
    rss_line("after decode");

    let t = Instant::now();
    let mut heights2 = preview::BlockHeights::default();
    preview_frame(&ctx, &doc, &pal, &mut images, &mut heights2, &mut hl, (1, 1), 0.0);
    println!("  warm (all cached)             {:>10.1} ms", t.elapsed().as_secs_f64() * 1000.0);

    // Re-decode a *different* corpus to force LRU eviction.
    let mut md_text2 = String::new();
    for i in 0..count {
        md_text2.push_str(&format!("![b2]({})\n\n", dir.join(format!("bench_{}.png", i + count)).display()));
    }
    for i in 0..count {
        let p = dir.join(format!("bench_{}.png", i + count));
        if !p.exists() {
            let mut buf = image::RgbImage::new(640, 480);
            for (x, y, px) in buf.enumerate_pixels_mut() {
                *px = image::Rgb([(x % 256) as u8, (y % 256) as u8, 200]);
            }
            buf.save(&p).unwrap();
        }
    }
    let doc2 = md::parse(&md_text2);
    let t = Instant::now();
    let mut heights3 = preview::BlockHeights::default();
    preview_frame(&ctx, &doc2, &pal, &mut images, &mut heights3, &mut hl, (2, 1), 0.0);
    println!("  second corpus (forces evict)  {:>10.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    let (cur2, peak2) = rss();
    println!(
        "  working set after eviction    {:>10.1} MiB (peak {:>8.1} MiB) — budget is 48 MiB",
        mib(cur2),
        mib(peak2)
    );
    let _ = (cur, peak);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sizes: Vec<usize> = if args.is_empty() {
        vec![1, 10]
    } else {
        args.iter().filter_map(|a| a.parse().ok()).collect()
    };
    let max_mb = sizes.iter().copied().max().unwrap_or(10);

    println!("RapidMD performance benchmark (headless)");
    rss_line("process start");

    bench_open(&sizes);
    bench_typing(max_mb.min(10), 120);
    bench_preview(max_mb.min(10), 40);
    bench_highlight();
    bench_images(std::path::Path::new("target/perf_imgs"), 48);

    bench_editor_split(10);
    bench_job_clone(10);
    bench_repaint(10);
    rss_line("process end");
}

// Extra: split the editor repaint cost into boundary-sort vs section-append.
fn bench_editor_split(mb: usize) {
    println!("\n=== 6. Editor repaint cost breakdown ({mb} MiB) ===");
    let text = synth(mb * 1024 * 1024);
    let font = egui::FontId::monospace(14.0);
    let mut tokens = Vec::new();
    highlight::tokenize_into(&text, &mut tokens);

    // Phase 1: build + sort + dedup the boundary vector.
    let t = Instant::now();
    let mut points = Vec::with_capacity(tokens.len() * 2 + 2);
    for (r, _) in tokens.iter() {
        points.push(r.start);
        points.push(r.end);
    }
    points.push(0);
    points.push(text.len());
    let sort_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("  boundary push                 {:>10.1} ms  ({} points)", sort_ms, points.len());
    let t = Instant::now();
    points.sort_unstable();
    points.dedup();
    println!("  boundary sort + dedup         {:>10.1} ms  ({} unique)", t.elapsed().as_secs_f64() * 1000.0, points.len());

    // Phase 2: the append loop, on the already-sorted vector.
    let t = Instant::now();
    let mut job = egui::text::LayoutJob::default();
    job.break_on_newline = true;
    let mut cursor = 0usize;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a == b { continue; }
        let seg = &text[a..b];
        if seg.is_empty() { continue; }
        while cursor < tokens.len() && tokens[cursor].0.end <= a { cursor += 1; }
        let kind = match tokens.get(cursor) {
            Some((r, k)) if r.start <= a && a < r.end => *k,
            _ => highlight::Tk::Plain,
        };
        let _ = kind;
        job.append(seg, 0.0, egui::text::TextFormat { font_id: font.clone(), ..Default::default() });
    }
    println!("  section append loop           {:>10.1} ms  ({} sections)", t.elapsed().as_secs_f64() * 1000.0, job.sections.len());
    rss_line("after breakdown");
}

// Extra: is cloning a cached LayoutJob cheaper than rebuilding it?
fn bench_job_clone(mb: usize) {
    println!("\n=== 7. Cached-job clone vs rebuild ({mb} MiB) ===");
    let text = synth(mb * 1024 * 1024);
    let pal = Palette::dark();
    let font = egui::FontId::monospace(14.0);
    let cache = std::cell::RefCell::new(editor::TokenCache::default());
    let mut scratch = editor::JobScratch::default();
    let built = editor::editor_job_cached_with(&text, &pal, &font, &[], &cache, (1, 1), &mut scratch);

    let mut rebuild = Stats::new();
    for _ in 0..10 {
        let t = Instant::now();
        let mut s2 = editor::JobScratch::default();
        let _j = editor::editor_job_cached_with(&text, &pal, &font, &[], &cache, (1, 1), &mut s2);
        rebuild.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    rebuild.report("rebuild (fresh scratch)", "ms");

    let mut clone = Stats::new();
    for _ in 0..10 {
        let t = Instant::now();
        let j = built.clone();
        clone.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&j);
    }
    clone.report("clone of cached job", "ms");
    println!("  sections {} text {} bytes", built.sections.len(), built.text.len());
}

// Extra: the repaint path the editor actually uses, cached vs not.
fn bench_repaint(mb: usize) {
    println!("\n=== 8. Editor repaint, cached vs uncached ({mb} MiB) ===");
    let mut text = synth(mb * 1024 * 1024);
    let pal = Palette::dark();
    let font = egui::FontId::monospace(14.0);
    let mut cache = editor::JobCache::default();
    let mut scratch = editor::JobScratch::default();

    // Cold: first build.
    let t = Instant::now();
    let _j = cache.job(&text, &pal, &font, &[], 1, (1, 1), 900.0, &mut scratch);
    println!("  cold build                   {:>10.1} ms", t.elapsed().as_secs_f64() * 1000.0);

    // Warm repaints: nothing changed (mouse move, hover, scroll reveal).
    let mut warm = Stats::new();
    for _ in 0..30 {
        let t = Instant::now();
        let j = cache.job(&text, &pal, &font, &[], 1, (1, 1), 900.0, &mut scratch);
        warm.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&j);
    }
    warm.report("warm repaint (cached job)", "ms");

    // Typing: revision bumps every keystroke, so it is a rebuild every time.
    let mut typing = Stats::new();
    for k in 0..20 {
        let at = (k * 977) % text.len().max(1);
        let at = (0..=at).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
        text.insert(at, 'x');
        let t = Instant::now();
        let j = cache.job(&text, &pal, &font, &[], 1, (1, 2 + k as u64), 900.0, &mut scratch);
        typing.push(t.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(&j);
    }
    typing.report("keystroke (rebuild)", "ms");
    rss_line("after repaint run");
}
