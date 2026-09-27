use std::time::Instant;

fn main() {
    let t = Instant::now();
    let set = syntect::parsing::SyntaxSet::load_defaults_newlines();
    let syntax_ms = t.elapsed().as_secs_f64() * 1000.0;
    let syntax_syntaxes = set.syntaxes().len();
    let syntax_bytes = std::mem::size_of_val(&set) as f64 / 1024.0;

    let t = Instant::now();
    let themes = syntect::highlighting::ThemeSet::load_defaults();
    let theme_ms = t.elapsed().as_secs_f64() * 1000.0;
    let theme_bytes = std::mem::size_of_val(&themes) as f64 / 1024.0;

    println!("SyntaxSet::load_defaults_newlines: {syntax_ms:.2} ms, {} syntaxes, struct {syntax_bytes:.1} KiB", syntax_syntaxes);
    println!("ThemeSet::load_defaults:            {theme_ms:.2} ms, {} themes, struct {theme_bytes:.1} KiB", themes.themes.len());
}
