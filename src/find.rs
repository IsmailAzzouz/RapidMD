//! Find & Replace state/logic (spec §6.16, F49).

/// Search state owned by the app.
#[derive(Default)]
pub struct Finder {
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub regex: bool,
    pub open: bool,
}

impl Finder {
    pub fn matches(&self, text: &str) -> Vec<std::ops::Range<usize>> {
        if self.query.is_empty() {
            return Vec::new();
        }
        let needle = &self.query;
        let hay = if self.case_sensitive { text.to_owned() } else { text.to_lowercase() };
        let nd = if self.case_sensitive { needle.to_owned() } else { needle.to_lowercase() };
        if nd.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut from = 0usize;
        while let Some(rel) = hay[from..].find(&nd) {
            let start = from + rel;
            let end = start + nd.len();
            out.push(start..end);
            if end == start {
                break;
            }
            from = end;
        }
        let _ = text;
        out
    }

    /// Replace the `nth` match with the replacement text; returns new text + byte range of replacement.
    pub fn replace_at(text: &str, m: &std::ops::Range<usize>, replacement: &str) -> String {
        let mut s = String::with_capacity(text.len() + replacement.len());
        s.push_str(&text[..m.start]);
        s.push_str(replacement);
        s.push_str(&text[m.end..]);
        s
    }

    /// Replace all matches as one atomic edit.
    pub fn replace_all(&self, text: &str) -> (String, usize) {
        let ms = self.matches(text);
        if ms.is_empty() {
            return (text.to_owned(), 0);
        }
        let mut out = String::with_capacity(text.len());
        let mut last = 0usize;
        for m in &ms {
            out.push_str(&text[last..m.start]);
            out.push_str(&self.replace);
            last = m.end;
        }
        out.push_str(&text[last..]);
        (out, ms.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_replaces() {
        let mut f = Finder::default();
        f.query = "foo".into();
        assert_eq!(f.matches("foo bar foo").len(), 2);
        f.replace = "X".into();
        let (t, n) = f.replace_all("foo bar foo");
        assert_eq!(n, 2);
        assert_eq!(t, "X bar X");
    }

    #[test]
    fn case_insensitive_default() {
        let f = Finder { query: "foo".into(), ..Default::default() };
        assert_eq!(f.matches("FOO foo").len(), 2);
    }
}
