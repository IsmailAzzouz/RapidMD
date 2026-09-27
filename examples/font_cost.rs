//! Measures what `egui::FontDefinitions::default()` costs versus what the app
//! actually installs, so unused default font data can be dropped safely.
//!
//! Run: `cargo run --profile fast --example font_cost`

use std::collections::HashSet;
use std::time::Instant;

use eframe::egui;

fn report(title: &str, fonts: &egui::FontDefinitions) {
    println!("\n== {title} ==");
    let total: usize = fonts.font_data.values().map(|f| f.font.len()).sum();
    for (name, data) in &fonts.font_data {
        println!("  {name:<24} {:>8.1} KiB", data.font.len() as f64 / 1024.0);
    }
    println!("  {:<24} {:>8.1} KiB", "TOTAL", total as f64 / 1024.0);

    for (family, stack) in &fonts.families {
        println!("  family {family:?}: {stack:?}");
    }

    // Which registered font data is not reachable from any family stack?
    let mut reachable: HashSet<&str> = HashSet::new();
    for stack in fonts.families.values() {
        for n in stack {
            reachable.insert(n.as_str());
        }
    }
    let mut orphans: Vec<&str> =
        fonts.font_data.keys().map(|k| k.as_str()).filter(|n| !reachable.contains(n)).collect();
    orphans.sort_unstable();
    println!("  unreachable (never rendered): {orphans:?}");
}

fn main() {
    let t = Instant::now();
    let default = egui::FontDefinitions::default();
    println!("egui::FontDefinitions::default(): {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    report("egui default", &default);

    let t = Instant::now();
    let app = rustdown_viewer::app::build_base_fonts();
    println!("\napp::build_base_fonts(): {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    report("app", &app);

    let d: usize = default.font_data.values().map(|f| f.font.len()).sum();
    let a: usize = app.font_data.values().map(|f| f.font.len()).sum();
    println!(
        "\nheap: default {:.1} KiB -> app {:.1} KiB (saved {:.1} KiB)",
        d as f64 / 1024.0,
        a as f64 / 1024.0,
        (d as f64 - a as f64) / 1024.0
    );
}
