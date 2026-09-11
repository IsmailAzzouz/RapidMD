//! Intermediate rich-text model + parser (pulldown-cmark → our blocks).
//!
//! Rendering-agnostic so it is unit-testable; the egui painter lives in
//! [`crate::preview`].

use pulldown_cmark::{Alignment, BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Text styling flags for inline spans.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

#[derive(Clone, Debug)]
pub enum Span {
    Text { text: String, style: Style },
    /// Inline code (monospace, tinted background).
    Code { text: String, style: Style },
    /// Clickable link; inner spans keep their styling.
    Link { url: String, title: Option<String>, inner: Vec<Span> },
    /// Image reference; `alt` doubles as tooltip.
    Image {
        url: String,
        alt: String,
        title: Option<String>,
        width: Option<f32>,
        height: Option<f32>,
    },
    /// Hard line break inside a paragraph.
    LineBreak,
    /// `[^label]` reference.
    FootnoteRef { label: String },
    /// `$...$` math — rendered monospaced/accented (no engine, documented).
    Math { text: String },
}

/// A blockquote variant (GFM callouts `[!NOTE]` etc. via ENABLE_GFM).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QuoteKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
    Plain,
}

impl QuoteKind {
    pub fn label(&self) -> Option<&'static str> {
        match self {
            QuoteKind::Plain => None,
            QuoteKind::Note => Some("NOTE"),
            QuoteKind::Tip => Some("TIP"),
            QuoteKind::Important => Some("IMPORTANT"),
            QuoteKind::Warning => Some("WARNING"),
            QuoteKind::Caution => Some("CAUTION"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColAlign {
    Left,
    Center,
    Right,
    None,
}

pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

pub struct Table {
    pub aligns: Vec<ColAlign>,
    pub header: Vec<Vec<Span>>,
    pub rows: Vec<Vec<Vec<Span>>>,
}

pub enum Block {
    Heading { level: u8, spans: Vec<Span> },
    Para(Vec<Span>),
    BlockQuote { kind: QuoteKind, items: Vec<Block> },
    List { ordered: bool, items: Vec<ListItem> },
    CodeBlock { lang: String, code: String },
    Table(Table),
    Rule,
    Html(String),
    FootnoteDef { label: String, blocks: Vec<Block> },
    Center(Vec<Block>),
}

pub struct Doc {
    pub blocks: Vec<Block>,
    /// (level, plain text, slug) for the TOC (spec F26).
    pub toc: Vec<(u8, String, String)>,
    /// Footnotes defined in the document, rendered at the end.
    pub footnotes: Vec<(String, Vec<Block>)>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn merge(base: Style, add: Style) -> Style {
    Style {
        bold: base.bold || add.bold,
        italic: base.italic || add.italic,
        strike: base.strike || add.strike,
        code: base.code || add.code,
    }
}

/// GitHub-style slug for heading anchors.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("section");
    }
    out
}

/// Plain-text projection of inline spans (TOC titles etc.).
pub fn spans_plain(spans: &[Span]) -> String {
    let mut s = String::new();
    for sp in spans {
        match sp {
            Span::Text { text, .. } => s.push_str(text),
            Span::Code { text, .. } => s.push_str(text),
            Span::Link { inner, .. } => s.push_str(&spans_plain(inner)),
            Span::Image { alt, .. } => s.push_str(alt),
            Span::LineBreak => s.push(' '),
            Span::FootnoteRef { label } => {
                s.push_str("[^");
                s.push_str(label);
                s.push(']');
            }
            Span::Math { text } => s.push_str(text),
        }
    }
    s
}

fn quote_kind(kind: Option<BlockQuoteKind>) -> QuoteKind {
    match kind {
        Some(BlockQuoteKind::Note) => QuoteKind::Note,
        Some(BlockQuoteKind::Tip) => QuoteKind::Tip,
        Some(BlockQuoteKind::Important) => QuoteKind::Important,
        Some(BlockQuoteKind::Warning) => QuoteKind::Warning,
        Some(BlockQuoteKind::Caution) => QuoteKind::Caution,
        _ => QuoteKind::Plain,
    }
}

/// End-of-container predicate factory (compares *kind*, ignores payloads that
/// may legitimately differ between Start/End, e.g. list ordering booleans).
fn same_kind(a: &TagEnd, b: &TagEnd) -> bool {
    use TagEnd as T;
    matches!(
        (a, b),
        (T::Paragraph, T::Paragraph)
            | (T::Heading(_), T::Heading(_))
            | (T::BlockQuote(_), T::BlockQuote(_))
            | (T::List(_), T::List(_))
            | (T::Item, T::Item)
            | (T::CodeBlock, T::CodeBlock)
            | (T::HtmlBlock, T::HtmlBlock)
            | (T::Table, T::Table)
            | (T::TableHead, T::TableHead)
            | (T::TableRow, T::TableRow)
            | (T::TableCell, T::TableCell)
            | (T::Emphasis, T::Emphasis)
            | (T::Strong, T::Strong)
            | (T::Strikethrough, T::Strikethrough)
            | (T::Link, T::Link)
            | (T::Image, T::Image)
            | (T::FootnoteDefinition, T::FootnoteDefinition)
            | (T::DefinitionList, T::DefinitionList)
            | (T::DefinitionListTitle, T::DefinitionListTitle)
            | (T::DefinitionListDefinition, T::DefinitionListDefinition)
    ) || std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Event queue with one-slot pushback (needed for task-list lookahead).
struct EvQueue<'a> {
    deque: VecDeque<Event<'a>>,
}

impl<'a> EvQueue<'a> {
    fn new(evs: Vec<Event<'a>>) -> Self {
        Self { deque: evs.into() }
    }
    fn next(&mut self) -> Option<Event<'a>> {
        self.deque.pop_front()
    }
    fn unshift(&mut self, ev: Event<'a>) {
        self.deque.push_front(ev);
    }
}

/// The end event we wait for, expressed as a closure over `&TagEnd`.
type EndFn = dyn Fn(&TagEnd) -> bool;

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse a full markdown document.
pub fn parse(text: &str) -> Doc {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    opts.insert(Options::ENABLE_GFM);

    let owned: Vec<Event<'static>> = Parser::new_ext(text, opts)
        .map(own_event)
        .collect();

    let mut evs = EvQueue::new(owned);
    let blocks = parse_blocks(&mut evs, &|_| false);

    let mut footnotes = Vec::new();
    let mut blocks_out = Vec::new();
    for b in blocks {
        match b {
            Block::FootnoteDef { label, blocks } => footnotes.push((label, blocks)),
            other => blocks_out.push(other),
        }
    }

    let mut toc = Vec::new();
    collect_toc(&blocks_out, &mut toc);

    Doc { blocks: blocks_out, toc, footnotes }
}

/// Owning conversion of parser events (CowStr → owned) so the document model
/// does not borrow the input text.
fn own_event(ev: Event<'_>) -> Event<'static> {
    fn own(c: pulldown_cmark::CowStr<'_>) -> pulldown_cmark::CowStr<'static> {
        pulldown_cmark::CowStr::Boxed(c.into_string().into_boxed_str())
    }
    match ev {
        Event::Start(tag) => Event::Start(match tag {
            Tag::Paragraph => Tag::Paragraph,
            Tag::Heading { level, id, classes, attrs } => Tag::Heading {
                level,
                id: id.map(|c| own(c)),
                classes: classes.into_iter().map(own).collect(),
                attrs: attrs.into_iter().map(|(a, b)| (own(a), b.map(own))).collect(),
            },
            Tag::BlockQuote(k) => Tag::BlockQuote(k),
            Tag::CodeBlock(k) => Tag::CodeBlock(match k {
                CodeBlockKind::Fenced(l) => CodeBlockKind::Fenced(own(l)),
                CodeBlockKind::Indented => CodeBlockKind::Indented,
            }),
            Tag::HtmlBlock => Tag::HtmlBlock,
            Tag::List(o) => Tag::List(o),
            Tag::Item => Tag::Item,
            Tag::FootnoteDefinition(l) => Tag::FootnoteDefinition(own(l)),
            Tag::DefinitionList => Tag::DefinitionList,
            Tag::DefinitionListTitle => Tag::DefinitionListTitle,
            Tag::DefinitionListDefinition => Tag::DefinitionListDefinition,
            Tag::Table(a) => Tag::Table(a),
            Tag::TableHead => Tag::TableHead,
            Tag::TableRow => Tag::TableRow,
            Tag::TableCell => Tag::TableCell,
            Tag::Emphasis => Tag::Emphasis,
            Tag::Strong => Tag::Strong,
            Tag::Strikethrough => Tag::Strikethrough,
            Tag::Link { link_type, dest_url, title, id } => Tag::Link {
                link_type,
                dest_url: own(dest_url),
                title: own(title),
                id: own(id),
            },
            Tag::Image { link_type, dest_url, title, id } => Tag::Image {
                link_type,
                dest_url: own(dest_url),
                title: own(title),
                id: own(id),
            },
            Tag::MetadataBlock(k) => Tag::MetadataBlock(k),
        }),
        Event::End(te) => Event::End(te),
        Event::Text(t) => Event::Text(own(t)),
        Event::Code(c) => Event::Code(own(c)),
        Event::InlineMath(m) => Event::InlineMath(own(m)),
        Event::DisplayMath(m) => Event::DisplayMath(own(m)),
        Event::Html(h) => Event::Html(own(h)),
        Event::InlineHtml(h) => Event::InlineHtml(own(h)),
        Event::FootnoteReference(l) => Event::FootnoteReference(own(l)),
        Event::SoftBreak => Event::SoftBreak,
        Event::HardBreak => Event::HardBreak,
        Event::Rule => Event::Rule,
        Event::TaskListMarker(c) => Event::TaskListMarker(c),
    }
}

fn parse_blocks(evs: &mut EvQueue<'_>, end: &EndFn) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut center_stack: Vec<Vec<Block>> = Vec::new();

    let push_b = |out: &mut Vec<Block>, center_stack: &mut Vec<Vec<Block>>, b: Block| {
        if let Some(target) = center_stack.last_mut() {
            target.push(b);
        } else {
            out.push(b);
        }
    };
    while let Some(ev) = evs.next() {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    let spans = parse_spans(evs, &|t| same_kind(t, &TagEnd::Paragraph));
                    push_b(&mut out, &mut center_stack, Block::Para(spans));
                }
                Tag::Heading { level, .. } => {
                    let lvl = level;
                    let spans = parse_spans(evs, &move |t| same_kind(t, &TagEnd::Heading(lvl)));
                    push_b(&mut out, &mut center_stack, Block::Heading { level: lvl as u8, spans });
                }
                Tag::BlockQuote(kind) => {
                    let kind = quote_kind(kind);
                    let items = parse_blocks(evs, &|t| same_kind(t, &TagEnd::BlockQuote(None)));
                    push_b(&mut out, &mut center_stack, Block::BlockQuote { kind, items });
                }
                Tag::List(start) => {
                    let ordered = start.is_some();
                    let mut items = Vec::new();
                    loop {
                        match evs.next() {
                            Some(Event::Start(Tag::Item)) => {
                                let mut checked = None;
                                if let Some(ev2) = evs.next() {
                                    match ev2 {
                                        Event::TaskListMarker(c) => checked = Some(c),
                                        other => evs.unshift(other),
                                    }
                                }
                                let blocks = parse_blocks(evs, &|t| same_kind(t, &TagEnd::Item));
                                items.push(ListItem { checked, blocks });
                            }
                            Some(Event::End(te)) if same_kind(&te, &TagEnd::List(false)) => break,
                            Some(Event::End(te)) if same_kind(&te, &TagEnd::Item) => {
                                // stray item end (already consumed) — ignore
                                let _ = te;
                            }
                            Some(_) => { /* loose text between items */ }
                            None => break,
                        }
                    }
                    push_b(&mut out, &mut center_stack, Block::List { ordered, items });
                }
                Tag::CodeBlock(kind) => {
                    let mut code = String::new();
                    while let Some(ev) = evs.next() {
                        match ev {
                            Event::Text(t) => code.push_str(&t),
                            Event::End(te) if same_kind(&te, &TagEnd::CodeBlock) => break,
                            _ => {}
                        }
                    }
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) => l.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    push_b(&mut out, &mut center_stack, Block::CodeBlock { lang, code });
                }
                Tag::Table(aligns) => {
                    let aligns: Vec<ColAlign> = aligns
                        .iter()
                        .map(|a| match a {
                            Alignment::None => ColAlign::None,
                            Alignment::Left => ColAlign::Left,
                            Alignment::Center => ColAlign::Center,
                            Alignment::Right => ColAlign::Right,
                        })
                        .collect();
                    let mut header: Vec<Vec<Span>> = Vec::new();
                    let mut rows: Vec<Vec<Vec<Span>>> = Vec::new();
                    let mut cur: Vec<Vec<Span>> = Vec::new();
                    let mut row_open = false;
                    loop {
                        match evs.next() {
                            Some(Event::Start(Tag::TableCell)) => {
                                let spans = parse_spans(evs, &|t| same_kind(t, &TagEnd::TableCell));
                                cur.push(spans);
                            }
                            Some(Event::Start(Tag::TableRow)) => {
                                row_open = true;
                                cur = Vec::new();
                            }
                            Some(Event::End(te)) if same_kind(&te, &TagEnd::TableHead) => {
                                // pulldown emits header cells directly under TableHead.
                                header = std::mem::take(&mut cur);
                                row_open = false;
                            }
                            Some(Event::End(te)) if same_kind(&te, &TagEnd::TableRow) => {
                                if row_open && !cur.is_empty() {
                                    rows.push(std::mem::take(&mut cur));
                                }
                                cur = Vec::new();
                                row_open = false;
                            }
                            Some(Event::End(te)) if same_kind(&te, &TagEnd::Table) => break,
                            _ => {}
                        }
                    }
                    // Safety for parsers that wrap the header row in TableRow.
                    if header.is_empty() && !rows.is_empty() {
                        header = rows.remove(0);
                    }
                    push_b(&mut out, &mut center_stack, Block::Table(Table { aligns, header, rows }));
                }
                Tag::FootnoteDefinition(label) => {
                    let blocks = parse_blocks(evs, &|t| same_kind(t, &TagEnd::FootnoteDefinition));
                    push_b(&mut out, &mut center_stack, Block::FootnoteDef { label: label.to_string(), blocks });
                }
                Tag::HtmlBlock => {
                    let mut html = String::new();
                    while let Some(ev) = evs.next() {
                        match ev {
                            Event::Html(h) => html.push_str(&h),
                            Event::End(te) if same_kind(&te, &TagEnd::HtmlBlock) => break,
                            _ => {}
                        }
                    }
                    let actions = crate::html::parse_html_block_content(&html);
                    for act in actions {
                        match act {
                            crate::html::HtmlBlockAction::OpenCenter => {
                                center_stack.push(Vec::new());
                            }
                            crate::html::HtmlBlockAction::CloseCenter => {
                                if let Some(children) = center_stack.pop() {
                                    if !children.is_empty() {
                                        push_b(&mut out, &mut center_stack, Block::Center(children));
                                    }
                                }
                            }
                            crate::html::HtmlBlockAction::Block(blk) => {
                                push_b(&mut out, &mut center_stack, blk);
                            }
                        }
                    }
                }
                tag @ (Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }) => {
                    // Loose inline content that never got a Paragraph container
                    // (pulldown omits it for tight list items).
                    evs.unshift(Event::Start(tag));
                    let spans = gather_inline_para(evs);
                    push_b(&mut out, &mut center_stack, Block::Para(spans));
                }
                _ => {
                    // Unknown container: skip defensively.
                }
            },
            Event::Rule => push_b(&mut out, &mut center_stack, Block::Rule),
            Event::End(te) => {
                if end(&te) {
                    break;
                }
            }
            ev @ (Event::Text(_)
            | Event::Code(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_)) => {
                // Tight list item content arrives as bare text without Paragraph.
                evs.unshift(ev);
                let spans = gather_inline_para(evs);
                push_b(&mut out, &mut center_stack, Block::Para(spans));
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                let actions = crate::html::parse_html_block_content(&h);
                for act in actions {
                    match act {
                        crate::html::HtmlBlockAction::OpenCenter => {
                            center_stack.push(Vec::new());
                        }
                        crate::html::HtmlBlockAction::CloseCenter => {
                            if let Some(children) = center_stack.pop() {
                                if !children.is_empty() {
                                    push_b(&mut out, &mut center_stack, Block::Center(children));
                                }
                            }
                        }
                        crate::html::HtmlBlockAction::Block(blk) => {
                            push_b(&mut out, &mut center_stack, blk);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    while let Some(children) = center_stack.pop() {
        if !children.is_empty() {
            push_b(&mut out, &mut center_stack, Block::Center(children));
        }
    }
    out
}

/// Gather inline events until a block boundary is reached; block-boundary
/// events are pushed back so the surrounding block parser can process them.
fn gather_inline_para(evs: &mut EvQueue<'_>) -> Vec<Span> {
    parse_spans_styled(evs, &|_| false, Style::default(), None)
}

fn is_block_start(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
    )
}

fn is_block_end(te: &TagEnd) -> bool {
    matches!(
        te,
        TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
    )
}

fn parse_spans(evs: &mut EvQueue<'_>, end: &EndFn) -> Vec<Span> {
    parse_spans_styled(evs, end, Style::default(), None)
}

fn parse_spans_styled(
    evs: &mut EvQueue<'_>,
    end: &EndFn,
    style: Style,
    link_url: Option<(String, Option<String>)>,
) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut current_style = style;
    let mut html_style_stack: Vec<Style> = Vec::new();
    while let Some(ev) = evs.next() {
        match ev {
            Event::Text(t) => {
                let decoded = crate::html::decode_entities(&t);
                if current_style.code {
                    out.push(Span::Code { text: decoded, style: current_style });
                } else {
                    out.push(Span::Text { text: decoded, style: current_style });
                }
            }
            Event::Code(c) => {
                let s = merge(current_style, Style { code: true, ..Style::default() });
                out.push(Span::Code { text: c.to_string(), style: s });
            }
            Event::SoftBreak => { /* spaces keep words apart; wrap does the rest */ }
            Event::HardBreak => out.push(Span::LineBreak),
            Event::InlineMath(m) => out.push(Span::Math { text: m.to_string() }),
            Event::DisplayMath(m) => out.push(Span::Math { text: m.to_string() }),
            Event::FootnoteReference(l) => out.push(Span::FootnoteRef { label: l.to_string() }),
            Event::InlineHtml(h) | Event::Html(h) => {
                let toks = crate::html::tokenize_html(&h);
                for tok in &toks {
                    match tok {
                        crate::html::HtmlToken::TagOpen { name, attrs, .. } => {
                            match name.as_str() {
                                "b" | "strong" => {
                                    html_style_stack.push(current_style);
                                    current_style.bold = true;
                                }
                                "i" | "em" => {
                                    html_style_stack.push(current_style);
                                    current_style.italic = true;
                                }
                                "s" | "del" | "strike" => {
                                    html_style_stack.push(current_style);
                                    current_style.strike = true;
                                }
                                "code" | "kbd" => {
                                    html_style_stack.push(current_style);
                                    current_style.code = true;
                                }
                                "br" => {
                                    out.push(Span::LineBreak);
                                }
                                "img" => {
                                    let url = crate::html::get_attr(attrs, "src").unwrap_or("").to_string();
                                    let alt = crate::html::get_attr(attrs, "alt").unwrap_or("").to_string();
                                    let title = crate::html::get_attr(attrs, "title").map(|s| s.to_string());
                                    let width = crate::html::get_attr(attrs, "width").and_then(crate::html::parse_dimension);
                                    let height = crate::html::get_attr(attrs, "height").and_then(crate::html::parse_dimension);
                                    out.push(Span::Image { url, alt, title, width, height });
                                }
                                _ => {}
                            }
                        }
                        crate::html::HtmlToken::TagClose { name } => {
                            if let Some(prev) = html_style_stack.pop() {
                                current_style = prev;
                            } else {
                                match name.as_str() {
                                    "b" | "strong" => current_style.bold = false,
                                    "i" | "em" => current_style.italic = false,
                                    "s" | "del" | "strike" => current_style.strike = false,
                                    "code" | "kbd" => current_style.code = false,
                                    _ => {}
                                }
                            }
                        }
                        crate::html::HtmlToken::Text(t) => {
                            if !t.is_empty() {
                                if current_style.code {
                                    out.push(Span::Code { text: t.clone(), style: current_style });
                                } else {
                                    out.push(Span::Text { text: t.clone(), style: current_style });
                                }
                            }
                        }
                    }
                }
            }
            Event::Start(tag) => match tag {
                Tag::Emphasis => {
                    let s = merge(style, Style { italic: true, ..Style::default() });
                    out.extend(parse_spans_styled(evs, &|t| same_kind(t, &TagEnd::Emphasis), s, link_url.clone()));
                }
                Tag::Strong => {
                    let s = merge(style, Style { bold: true, ..Style::default() });
                    out.extend(parse_spans_styled(evs, &|t| same_kind(t, &TagEnd::Strong), s, link_url.clone()));
                }
                Tag::Strikethrough => {
                    let s = merge(style, Style { strike: true, ..Style::default() });
                    out.extend(parse_spans_styled(evs, &|t| same_kind(t, &TagEnd::Strikethrough), s, link_url.clone()));
                }
                Tag::Link { dest_url, title, .. } => {
                    let url = dest_url.to_string();
                    let title = if title.is_empty() { None } else { Some(title.to_string()) };
                    let endf = |t: &TagEnd| same_kind(t, &TagEnd::Link);
                    if link_url.is_some() {
                        // Nested link inside a link: treat its text as plain.
                        let inner = parse_spans_styled(evs, &endf, style, link_url.clone());
                        out.extend(inner);
                    } else {
                        let inner = parse_spans_styled(evs, &endf, style, Some((url.clone(), title.clone())));
                        out.push(Span::Link { url, title, inner });
                    }
                }
                Tag::Image { dest_url, title, .. } => {
                    let mut alt = String::new();
                    while let Some(ev2) = evs.next() {
                        match ev2 {
                            Event::Text(t) => alt.push_str(&t),
                            Event::Code(c) => alt.push_str(&c),
                            Event::End(te) if same_kind(&te, &TagEnd::Image) => break,
                            _ => {}
                        }
                    }
                    let title = if title.is_empty() { None } else { Some(title.to_string()) };
                    out.push(Span::Image {
                        url: dest_url.to_string(),
                        alt,
                        title,
                        width: None,
                        height: None,
                    });
                }
                tag => {
                    if is_block_start(&tag) {
                        // A new block began while collecting inline runs.
                        evs.unshift(Event::Start(tag));
                        break;
                    }
                    // Unknown inline container: skip defensively.
                }
            },
            Event::End(te) => {
                if end(&te) {
                    break;
                }
                if is_block_end(&te) {
                    // Belongs to a parent parser — hand it back.
                    evs.unshift(Event::End(te));
                    break;
                }
            }
            _ => {}
        }
    }
    out
}

fn collect_toc(blocks: &[Block], toc: &mut Vec<(u8, String, String)>) {
    fn walk(blocks: &[Block], toc: &mut Vec<(u8, String, String)>, seen: &mut std::collections::HashMap<String, usize>) {
        for b in blocks {
            match b {
                Block::Heading { level, spans } => {
                    let title = spans_plain(spans);
                    let base = slugify(&title);
                    let count = seen.entry(base.clone()).or_insert(0);
                    let slug = if *count == 0 { base } else { format!("{}-{}", base, count) };
                    *count += 1;
                    toc.push((*level, title, slug));
                }
                Block::BlockQuote { items, .. } => walk(items, toc, seen),
                Block::List { items, .. } => {
                    for item in items {
                        walk(&item.blocks, toc, seen);
                    }
                }
                Block::Center(items) => walk(items, toc, seen),
                _ => {}
            }
        }
    }
    walk(blocks, toc, &mut std::collections::HashMap::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_heading_with_slug() {
        let doc = parse("# Hello World\n\nSome *text*.");
        assert_eq!(doc.toc.len(), 1);
        assert_eq!(doc.toc[0], (1, "Hello World".to_owned(), "hello-world".to_owned()));
        assert!(matches!(&doc.blocks[1], Block::Para(_)));
    }

    #[test]
    fn parses_table() {
        let md = "| A | B |\n| --- | --- |\n| 1 | 2 |\n";
        let doc = parse(md);
        let Block::Table(t) = &doc.blocks[0] else { panic!("expected table") };
        assert_eq!(t.header.len(), 2);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(spans_plain(&t.rows[0][0]), "1");
    }

    #[test]
    fn parses_task_list() {
        let md = "- [x] done\n- [ ] todo\n";
        let doc = parse(md);
        let Block::List { ordered, items, .. } = &doc.blocks[0] else { panic!("expected list") };
        assert!(!ordered);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].checked, Some(true));
        assert_eq!(items[1].checked, Some(false));
    }

    #[test]
    fn parses_bold_and_link() {
        let md = "**bold** and [a link](https://x.dev)";
        let doc = parse(md);
        let Block::Para(spans) = &doc.blocks[0] else { panic!("expected para") };
        let has_bold = spans.iter().any(|s| matches!(s, Span::Text { style, .. } if style.bold));
        assert!(has_bold, "expected a bold text run: {:?}", spans);
        assert!(spans.iter().any(|s| matches!(s, Span::Link { url, .. } if url == "https://x.dev")));
    }

    #[test]
    fn slugify_works() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        // Non-ASCII letters survive (GitHub keeps them); only punctuation folds.
        assert_eq!(slugify("Über naïve"), "über-naïve");
    }

    #[test]
    fn parses_footnotes() {
        let md = "Text[^1].\n\n[^1]: footnote body";
        let doc = parse(md);
        assert_eq!(doc.footnotes.len(), 1);
        assert_eq!(doc.footnotes[0].0, "1");
    }

    #[test]
    fn parses_plain_paragraph_after_list_marker_lookahead() {
        let md = "- first\n- second\n";
        let doc = parse(md);
        let Block::List { items, .. } = &doc.blocks[0] else { panic!("expected list") };
        assert_eq!(items.len(), 2);
        assert!(!items[0].blocks.is_empty());
    }

    #[test]
    fn parses_blockquote_and_code() {
        let md = "> quoted\n\n```rust\nfn main() {}\n```\n";
        let doc = parse(md);
        assert!(matches!(&doc.blocks[0], Block::BlockQuote { .. }));
        assert!(matches!(&doc.blocks[1], Block::CodeBlock { lang, .. } if lang == "rust"));
    }

    #[test]
    fn parses_images_various() {
        let md2 = "![alt](C:/Users/ismai/Desktop/image.png)";
        let doc2 = parse(md2);
        let Block::Para(spans2) = &doc2.blocks[0] else { panic!() };
        assert!(matches!(&spans2[0], Span::Image { url, .. } if url == "C:/Users/ismai/Desktop/image.png"));

        let md3 = "![](image.png)";
        let doc3 = parse(md3);
        let Block::Para(spans3) = &doc3.blocks[0] else { panic!() };
        assert!(matches!(&spans3[0], Span::Image { url, .. } if url == "image.png"));

        let md6 = "![alt](<C:\\Users\\ismai\\My Images\\pic.png>)";
        let doc6 = parse(md6);
        let Block::Para(spans6) = &doc6.blocks[0] else { panic!() };
        assert!(matches!(&spans6[0], Span::Image { .. }));
    }

    #[test]
    fn test_readme_header_full_parse() {
        let md = r#"# RapidMD ⚡

<div align="center">
  <img src="assets/Icon/RMD.png" alt="RapidMD Logo" width="96" height="96" style="border-radius: 18px;" />
  <h3>Sleek, Native Markdown Viewer & Editor in Rust</h3>
  <p>Minimalist monochrome interface and workspace aesthetics.</p>

  [![Rust](https://img.shields.io/badge/Rust-2021%20Edition-black?logo=rust)](https://www.rust-lang.org/)
</div>

---
"#;
        let doc = parse(md);
        assert_eq!(doc.blocks.len(), 3); // H1, Center, Rule
        assert!(matches!(&doc.blocks[0], Block::Heading { level: 1, .. }));
        let Block::Center(center_items) = &doc.blocks[1] else { panic!("expected Center block") };
        assert_eq!(center_items.len(), 4); // image para, H3, description para, badges para
        assert!(matches!(&center_items[0], Block::Para(spans) if matches!(&spans[0], Span::Image { width: Some(96.0), height: Some(96.0), .. })));
        assert!(matches!(&center_items[1], Block::Heading { level: 3, .. }));
        assert!(matches!(&center_items[2], Block::Para(_)));
        assert!(matches!(&center_items[3], Block::Para(_)));
        assert!(matches!(&doc.blocks[2], Block::Rule));
    }

    #[test]
    fn test_inline_html_in_markdown() {
        let md = "Press <kbd>Ctrl+S</kbd> to save. Here is <b>bold</b> text.";
        let doc = parse(md);
        let Block::Para(spans) = &doc.blocks[0] else { panic!("expected para") };
        assert!(spans.iter().any(|s| matches!(s, Span::Code { text, .. } if text == "Ctrl+S")));
        assert!(spans.iter().any(|s| matches!(s, Span::Text { style, text } if style.bold && text == "bold")));
    }
}
