//! Zero-dependency HTML compatibility parser for Markdown (HTML blocks & inline tags).

use crate::md::{Block, QuoteKind, Span, Style};

/// Decodes common HTML entities into UTF-8 characters.
pub fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            let mut closed = false;
            while let Some(&ec) = chars.peek() {
                if ec == ';' {
                    chars.next();
                    closed = true;
                    break;
                }
                if ec == '&' || ec == '<' || ec == '>' || ec == ' ' || entity.len() > 10 {
                    break;
                }
                entity.push(chars.next().unwrap());
            }
            if closed {
                match entity.as_str() {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" | "#39" => out.push('\''),
                    "nbsp" => out.push(' '),
                    _ => {
                        if let Some(code) = entity.strip_prefix('#') {
                            let parsed = if let Some(hex) = code.strip_prefix('x').or_else(|| code.strip_prefix('X')) {
                                u32::from_str_radix(hex, 16).ok()
                            } else {
                                code.parse::<u32>().ok()
                            };
                            if let Some(ch) = parsed.and_then(char::from_u32) {
                                out.push(ch);
                            } else {
                                out.push('&');
                                out.push_str(&entity);
                                out.push(';');
                            }
                        } else {
                            out.push('&');
                            out.push_str(&entity);
                            out.push(';');
                        }
                    }
                }
            } else {
                out.push('&');
                out.push_str(&entity);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parses dimension attribute like `"96"` or `"96px"`.
pub fn parse_dimension(val: &str) -> Option<f32> {
    let s = val.trim().trim_end_matches("px").trim();
    s.parse::<f32>().ok()
}

#[derive(Debug, Clone, PartialEq)]
pub enum HtmlToken {
    TagOpen {
        name: String,
        attrs: Vec<(String, String)>,
        self_closing: bool,
    },
    TagClose {
        name: String,
    },
    Text(String),
}

/// Extracts attributes from tag content after the tag name.
pub fn extract_attrs(attr_str: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let chars: Vec<char> = attr_str.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }

        // Read attribute name
        let name_start = i;
        while i < len && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '/' && chars[i] != '>' {
            i += 1;
        }
        let name: String = chars[name_start..i].iter().collect();
        if name.is_empty() {
            i += 1;
            continue;
        }

        // Skip whitespace
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }

        let mut val = String::new();
        if i < len && chars[i] == '=' {
            i += 1;
            // Skip whitespace after =
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            if i < len {
                let quote = chars[i];
                if quote == '"' || quote == '\'' {
                    i += 1;
                    let val_start = i;
                    while i < len && chars[i] != quote {
                        i += 1;
                    }
                    val = chars[val_start..i].iter().collect();
                    if i < len && chars[i] == quote {
                        i += 1;
                    }
                } else {
                    let val_start = i;
                    while i < len && !chars[i].is_whitespace() && chars[i] != '>' && chars[i] != '/' {
                        i += 1;
                    }
                    val = chars[val_start..i].iter().collect();
                }
            }
        }
        attrs.push((name.to_ascii_lowercase(), decode_entities(&val)));
    }
    attrs
}

/// Tokenizes HTML text into structured tokens.
pub fn tokenize_html(html: &str) -> Vec<HtmlToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '<' {
            // Check for comment <!-- ... -->
            if i + 3 < len && chars[i + 1] == '!' && chars[i + 2] == '-' && chars[i + 3] == '-' {
                let mut end = i + 4;
                while end + 2 < len {
                    if chars[end] == '-' && chars[end + 1] == '-' && chars[end + 2] == '>' {
                        end += 3;
                        break;
                    }
                    end += 1;
                }
                i = end;
                continue;
            }

            // Find matching '>'
            let mut end = i + 1;
            while end < len && chars[end] != '>' {
                end += 1;
            }
            if end >= len {
                let text: String = chars[i..].iter().collect();
                tokens.push(HtmlToken::Text(decode_entities(&text)));
                break;
            }

            let tag_content: String = chars[i + 1..end].iter().collect();
            i = end + 1;
            let trimmed = tag_content.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(close_name) = trimmed.strip_prefix('/') {
                let name = close_name.trim().split_whitespace().next().unwrap_or("").to_ascii_lowercase();
                if !name.is_empty() {
                    tokens.push(HtmlToken::TagClose { name });
                }
            } else {
                let self_closing = trimmed.ends_with('/');
                let inner = if self_closing {
                    trimmed.trim_end_matches('/').trim()
                } else {
                    trimmed
                };
                let mut parts = inner.splitn(2, char::is_whitespace);
                let tag_name = parts.next().unwrap_or("").to_ascii_lowercase();
                let attr_str = parts.next().unwrap_or("");
                let attrs = extract_attrs(attr_str);
                let is_void = matches!(tag_name.as_str(), "img" | "br" | "hr" | "input" | "meta" | "link");
                tokens.push(HtmlToken::TagOpen {
                    name: tag_name,
                    attrs,
                    self_closing: self_closing || is_void,
                });
            }
        } else {
            let start = i;
            while i < len && chars[i] != '<' {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            if !text.is_empty() {
                tokens.push(HtmlToken::Text(decode_entities(&text)));
            }
        }
    }
    tokens
}

/// Helper to get an attribute value from an attribute list.
pub fn get_attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Checks if an open tag indicates center alignment.
pub fn is_center_align(name: &str, attrs: &[(String, String)]) -> bool {
    if matches!(name, "center") {
        return true;
    }
    if let Some(align) = get_attr(attrs, "align") {
        if align.eq_ignore_ascii_case("center") {
            return true;
        }
    }
    if let Some(style) = get_attr(attrs, "style") {
        let clean = style.to_ascii_lowercase().replace(' ', "");
        if clean.contains("text-align:center") {
            return true;
        }
    }
    false
}

/// Parses inline HTML tokens into a list of Spans.
pub fn parse_inline_tokens(tokens: &[HtmlToken], base_style: Style) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut style_stack = vec![base_style];
    let mut link_stack: Vec<(String, Option<String>, usize)> = Vec::new();

    for tok in tokens {
        match tok {
            HtmlToken::TagOpen { name, attrs, .. } => {
                match name.as_str() {
                    "b" | "strong" => {
                        let mut s = *style_stack.last().unwrap();
                        s.bold = true;
                        style_stack.push(s);
                    }
                    "i" | "em" => {
                        let mut s = *style_stack.last().unwrap();
                        s.italic = true;
                        style_stack.push(s);
                    }
                    "s" | "del" | "strike" => {
                        let mut s = *style_stack.last().unwrap();
                        s.strike = true;
                        style_stack.push(s);
                    }
                    "code" | "kbd" => {
                        let mut s = *style_stack.last().unwrap();
                        s.code = true;
                        style_stack.push(s);
                    }
                    "u" | "mark" | "span" | "small" | "sub" | "sup" => {
                        let s = *style_stack.last().unwrap();
                        style_stack.push(s);
                    }
                    "br" => {
                        spans.push(Span::LineBreak);
                    }
                    "img" => {
                        let url = get_attr(attrs, "src").unwrap_or("").to_string();
                        let alt = get_attr(attrs, "alt").unwrap_or("").to_string();
                        let title = get_attr(attrs, "title").map(|s| s.to_string());
                        let width = get_attr(attrs, "width").and_then(parse_dimension);
                        let height = get_attr(attrs, "height").and_then(parse_dimension);
                        spans.push(Span::Image {
                            url,
                            alt,
                            title,
                            width,
                            height,
                        });
                    }
                    "a" => {
                        let href = get_attr(attrs, "href").unwrap_or("").to_string();
                        let title = get_attr(attrs, "title").map(|s| s.to_string());
                        link_stack.push((href, title, spans.len()));
                    }
                    _ => {}
                }
            }
            HtmlToken::TagClose { name } => {
                match name.as_str() {
                    "b" | "strong" | "i" | "em" | "s" | "del" | "strike" | "code" | "kbd" | "u" | "mark" | "span" | "small" | "sub" | "sup" => {
                        if style_stack.len() > 1 {
                            style_stack.pop();
                        }
                    }
                    "a" => {
                        if let Some((url, title, start_idx)) = link_stack.pop() {
                            let inner = spans.split_off(start_idx);
                            spans.push(Span::Link { url, title, inner });
                        }
                    }
                    _ => {}
                }
            }
            HtmlToken::Text(text) => {
                if !text.is_empty() {
                    let s = *style_stack.last().unwrap();
                    if s.code {
                        spans.push(Span::Code {
                            text: text.clone(),
                            style: s,
                        });
                    } else {
                        spans.push(Span::Text {
                            text: text.clone(),
                            style: s,
                        });
                    }
                }
            }
        }
    }

    // Close any unclosed links gracefully
    while let Some((url, title, start_idx)) = link_stack.pop() {
        let inner = spans.split_off(start_idx);
        spans.push(Span::Link { url, title, inner });
    }

    spans
}

/// Block action result from parsing an HTML block.
pub enum HtmlBlockAction {
    Block(Block),
    OpenCenter,
    CloseCenter,
}

/// Parses a raw HTML block into high-level Markdown blocks and center container triggers.
pub fn parse_html_block_content(html: &str) -> Vec<HtmlBlockAction> {
    let tokens = tokenize_html(html);
    let mut actions = Vec::new();
    let mut i = 0;
    let n = tokens.len();

    while i < n {
        match &tokens[i] {
            HtmlToken::TagOpen { name, attrs, self_closing: _ } => {
                let name_str = name.as_str();
                if is_center_align(name_str, attrs) {
                    actions.push(HtmlBlockAction::OpenCenter);
                    i += 1;
                    continue;
                }
                if name_str == "div" {
                    // Regular div without center alignment
                    i += 1;
                    continue;
                }
                if name_str == "hr" {
                    actions.push(HtmlBlockAction::Block(Block::Rule));
                    i += 1;
                    continue;
                }
                if name_str == "img" {
                    let url = get_attr(attrs, "src").unwrap_or("").to_string();
                    let alt = get_attr(attrs, "alt").unwrap_or("").to_string();
                    let title = get_attr(attrs, "title").map(|s| s.to_string());
                    let width = get_attr(attrs, "width").and_then(parse_dimension);
                    let height = get_attr(attrs, "height").and_then(parse_dimension);
                    actions.push(HtmlBlockAction::Block(Block::Para(vec![Span::Image {
                        url,
                        alt,
                        title,
                        width,
                        height,
                    }])));
                    i += 1;
                    continue;
                }
                if name_str.len() == 2 && name_str.starts_with('h') && name_str[1..].chars().all(|c| ('1'..='6').contains(&c)) {
                    let level = name_str[1..].parse::<u8>().unwrap_or(1);
                    i += 1;
                    let mut inner_tokens = Vec::new();
                    while i < n {
                        match &tokens[i] {
                            HtmlToken::TagClose { name: close_name } if close_name == name_str => {
                                i += 1;
                                break;
                            }
                            HtmlToken::TagOpen { name: other, .. } if other.starts_with('h') || other == "p" || other == "div" => {
                                // Another block started without explicit close
                                break;
                            }
                            other => {
                                inner_tokens.push(other.clone());
                                i += 1;
                            }
                        }
                    }
                    let spans = parse_inline_tokens(&inner_tokens, Style::default());
                    if !spans.is_empty() {
                        actions.push(HtmlBlockAction::Block(Block::Heading { level, spans }));
                    }
                    continue;
                }
                if name_str == "p" {
                    i += 1;
                    let mut inner_tokens = Vec::new();
                    while i < n {
                        match &tokens[i] {
                            HtmlToken::TagClose { name: close_name } if close_name == "p" => {
                                i += 1;
                                break;
                            }
                            HtmlToken::TagOpen { name: other, .. } if other.starts_with('h') || other == "p" || other == "div" => {
                                break;
                            }
                            other => {
                                inner_tokens.push(other.clone());
                                i += 1;
                            }
                        }
                    }
                    let spans = parse_inline_tokens(&inner_tokens, Style::default());
                    if !spans.is_empty() {
                        actions.push(HtmlBlockAction::Block(Block::Para(spans)));
                    }
                    continue;
                }
                if name_str == "blockquote" {
                    i += 1;
                    let mut inner_tokens = Vec::new();
                    while i < n {
                        match &tokens[i] {
                            HtmlToken::TagClose { name: close_name } if close_name == "blockquote" => {
                                i += 1;
                                break;
                            }
                            other => {
                                inner_tokens.push(other.clone());
                                i += 1;
                            }
                        }
                    }
                    let spans = parse_inline_tokens(&inner_tokens, Style::default());
                    if !spans.is_empty() {
                        actions.push(HtmlBlockAction::Block(Block::BlockQuote {
                            kind: QuoteKind::Plain,
                            items: vec![Block::Para(spans)],
                        }));
                    }
                    continue;
                }
                if name_str == "pre" {
                    i += 1;
                    let mut code = String::new();
                    let mut lang = String::new();
                    while i < n {
                        match &tokens[i] {
                            HtmlToken::TagClose { name: close_name } if close_name == "pre" => {
                                i += 1;
                                break;
                            }
                            HtmlToken::TagOpen { name: code_tag, attrs, .. } if code_tag == "code" => {
                                if let Some(class) = get_attr(attrs, "class") {
                                    if let Some(l) = class.strip_prefix("language-").or_else(|| class.strip_prefix("lang-")) {
                                        lang = l.to_string();
                                    }
                                }
                                i += 1;
                            }
                            HtmlToken::Text(t) => {
                                code.push_str(t);
                                i += 1;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    }
                    actions.push(HtmlBlockAction::Block(Block::CodeBlock { lang, code }));
                    continue;
                }

                // Any inline tag at root level (e.g. <a>, <b>, <span>, etc.)
                let mut inline_tokens = vec![tokens[i].clone()];
                i += 1;
                while i < n {
                    match &tokens[i] {
                        HtmlToken::TagOpen { name: blk, .. } if matches!(blk.as_str(), "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "hr" | "pre" | "blockquote") => {
                            break;
                        }
                        HtmlToken::TagClose { name: blk } if matches!(blk.as_str(), "div" | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote") => {
                            break;
                        }
                        other => {
                            inline_tokens.push(other.clone());
                            i += 1;
                        }
                    }
                }
                let spans = parse_inline_tokens(&inline_tokens, Style::default());
                let has_visible = spans.iter().any(|s| match s {
                    Span::Text { text, .. } => !text.trim().is_empty(),
                    _ => true,
                });
                if has_visible {
                    actions.push(HtmlBlockAction::Block(Block::Para(spans)));
                }
            }
            HtmlToken::TagClose { name } => {
                if name == "div" || name == "center" {
                    actions.push(HtmlBlockAction::CloseCenter);
                }
                i += 1;
            }
            HtmlToken::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    actions.push(HtmlBlockAction::Block(Block::Para(vec![Span::Text {
                        text: text.clone(),
                        style: Style::default(),
                    }])));
                }
                i += 1;
            }
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_entities() {
        assert_eq!(decode_entities("&lt;div&gt; &amp; &quot;hi&quot; &#39;test&#39;"), "<div> & \"hi\" 'test'");
        assert_eq!(decode_entities("Hello&nbsp;World"), "Hello World");
    }

    #[test]
    fn test_parse_dimension() {
        assert_eq!(parse_dimension("96"), Some(96.0));
        assert_eq!(parse_dimension("96px"), Some(96.0));
        assert_eq!(parse_dimension(" 120.5 px "), Some(120.5));
        assert_eq!(parse_dimension("auto"), None);
    }

    #[test]
    fn test_extract_attrs() {
        let attrs = extract_attrs(r#"src="assets/Icon/RMD.png" alt="RapidMD Logo" width="96" height="96" style="border-radius: 18px;""#);
        assert_eq!(get_attr(&attrs, "src"), Some("assets/Icon/RMD.png"));
        assert_eq!(get_attr(&attrs, "alt"), Some("RapidMD Logo"));
        assert_eq!(get_attr(&attrs, "width"), Some("96"));
        assert_eq!(get_attr(&attrs, "height"), Some("96"));
    }

    #[test]
    fn test_readme_header_html_block() {
        let html = r#"<div align="center">
  <img src="assets/Icon/RMD.png" alt="RapidMD Logo" width="96" height="96" style="border-radius: 18px;" />
  <h3>Sleek, Native Markdown Viewer & Editor in Rust</h3>
  <p>Minimalist monochrome interface and workspace aesthetics.</p>
</div>"#;
        let actions = parse_html_block_content(html);
        assert!(matches!(actions[0], HtmlBlockAction::OpenCenter));
        match &actions[1] {
            HtmlBlockAction::Block(Block::Para(spans)) => {
                assert!(matches!(&spans[0], Span::Image { url, width: Some(96.0), height: Some(96.0), .. } if url == "assets/Icon/RMD.png"));
            }
            _ => panic!("expected image para"),
        }
        match &actions[2] {
            HtmlBlockAction::Block(Block::Heading { level: 3, spans }) => {
                assert_eq!(crate::md::spans_plain(spans), "Sleek, Native Markdown Viewer & Editor in Rust");
            }
            _ => panic!("expected h3 heading"),
        }
        match &actions[3] {
            HtmlBlockAction::Block(Block::Para(spans)) => {
                assert_eq!(crate::md::spans_plain(spans), "Minimalist monochrome interface and workspace aesthetics.");
            }
            _ => panic!("expected paragraph"),
        }
        assert!(matches!(actions[4], HtmlBlockAction::CloseCenter));
    }

    #[test]
    fn test_html_comments_are_ignored() {
        let html = "<!-- this is a comment --><p>visible</p>";
        let actions = parse_html_block_content(html);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], HtmlBlockAction::Block(Block::Para(_))));
    }
}
