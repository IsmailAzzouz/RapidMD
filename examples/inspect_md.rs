use pulldown_cmark::{Options, Parser};

fn main() {
    let md = std::env::args().nth(1).unwrap_or_else(|| "| A | B |\n| --- | --- |\n| 1 | 2 |\n".into());
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    opts.insert(Options::ENABLE_GFM);
    for ev in Parser::new_ext(&md, opts) {
        println!("{:?}", ev);
    }
}
