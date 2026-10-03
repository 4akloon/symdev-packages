//! `PyText`: the few `str` operations of Python 3 the tools were written with, so that
//! the Rust tools cut, strip and split text exactly as the Python ones did.

/// Python's whitespace (`str.isspace`, `\s`, `strip`) and line breaks (`splitlines`).
pub struct PyText;

impl PyText {
    /// Python's whitespace: Unicode White_Space plus the separators U+001C..U+001F.
    pub fn is_space(c: char) -> bool {
        c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
    }

    pub fn strip(text: &str) -> &str {
        text.trim_matches(Self::is_space)
    }

    pub fn lstrip(text: &str) -> &str {
        text.trim_start_matches(Self::is_space)
    }

    pub fn rstrip(text: &str) -> &str {
        text.trim_end_matches(Self::is_space)
    }

    /// `re.sub(r"\s+", " ", text)`.
    pub fn collapse_spaces(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut in_space = false;
        for c in text.chars() {
            if Self::is_space(c) {
                if !in_space {
                    out.push(' ');
                }
                in_space = true;
            } else {
                out.push(c);
                in_space = false;
            }
        }
        out
    }

    /// `str.splitlines()`: no trailing empty line; `\r\n` is one break.
    pub fn split_lines(text: &str) -> Vec<&str> {
        let mut lines = Vec::new();
        let mut start = 0;
        let mut chars = text.char_indices().peekable();
        while let Some((at, c)) = chars.next() {
            if !matches!(
                c,
                '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'
                    ..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
            ) {
                continue;
            }
            lines.push(&text[start..at]);
            start = at + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|&(_, next)| next == '\n') {
                chars.next();
                start += 1;
            }
        }
        if start < text.len() {
            lines.push(&text[start..]);
        }
        lines
    }

    /// What Python's text mode reads: `\r\n` and a lone `\r` become `\n`.
    pub fn universal_newlines(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::PyText;

    #[test]
    fn whitespace_is_pythons() {
        assert!(PyText::is_space('\u{1f}') && PyText::is_space('\u{a0}'));
        assert!(!PyText::is_space('\u{200b}'));
        assert_eq!(PyText::strip("\u{1c} a b \u{3000}"), "a b");
        assert_eq!(PyText::collapse_spaces("a \t\n b  c"), "a b c");
    }

    #[test]
    fn lines_split_as_splitlines_does() {
        assert_eq!(
            PyText::split_lines("a\r\nb\rc\n\nd\u{2028}e\n"),
            ["a", "b", "c", "", "d", "e"]
        );
        assert_eq!(PyText::split_lines(""), Vec::<&str>::new());
        assert_eq!(PyText::universal_newlines("a\r\nb\rc"), "a\nb\nc");
    }
}
